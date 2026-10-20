//! Segments of a grouped alternative, over the storages a query borrows.
//!
//! [`grouped_entities`] publishes the walks [`select`](super::select) estimated as a plan:
//! one segment per piece of a walk, each with the row it installs and the regions earlier
//! pieces own. Walks already split at trust endpoints; placement also cuts them wherever
//! one of their active captains' rings wraps. Neither cut changes the covered logical positions.\
//! It doesn't read entities, segments are carved out of the members' entities from metadata alone.
//!
//! The group can be held by a nested `Mixed`. Each `Mixed` between the root and that one then
//! drives every segment through the single child leading to it, the rest of the query is looked up.

use super::{Alternative, GroupedCandidate, Member, Route, Walk, mask_children};
use crate::iter::plan::{
    Coverage, CoverageSpan, NodeKind, NodeState, Plan, SegmentDescriptor, SegmentEntry, SegmentRow,
    SegmentSchedule,
};
use crate::iter::{EntityCursor, RawEntityIdAccess, Shiperator};
use crate::sparse_set::Ring;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;

/// Returns the entities driving `root` through every walk of `alternative`, with `root`
/// installed for the first one.
///
/// `root` is the `Mixed` that reported `alternative`, in the state its constructor built.
/// It must be the query's root: rows are built for its whole layout.\
/// `root`'s new state points into the plan held by the returned entities, they have to drive it.
///
/// When `alternative` has no walk, the returned entities have no candidate and no plan,
/// `root` is left as is.
pub(crate) fn grouped_entities<S: Shiperator>(
    root: &mut S,
    alternative: &Alternative,
) -> RawEntityIdAccess {
    let Some(plan) = grouped_plan(root, alternative) else {
        return RawEntityIdAccess::empty();
    };

    let mut schedule = SegmentSchedule::new(plan);
    let first = schedule
        .take_next()
        .expect("plans have at least one segment");

    // The first segment is entered like any follow-up, from its row in the published plan.
    let mut entry = SegmentEntry::new(schedule.plan(), &first);
    // SAFE: `IntoShiperator`'s contract ties `alternative`'s metadata and member entities to
    // the storages `root`'s windows borrow. `grouped_plan` built every row for `root`'s own
    // layout, from `root`'s description. In each segment, a captain
    // window starts at the index, in its own storage, of the segment's first entity and the
    // segment fits in that storage. Segments stop at every captain's wrap point, so each
    // captain's entities are consecutive from there. With several captains, this relies on
    // members holding the same entities at the logical positions they all trust, which their
    // group metadata guarantees.
    // The `Mixed`s above the group's owner only have the child leading to it as captain, they
    // pass the candidates down unchanged.
    // `root` gets this segment's candidates from `first`'s cursor, which we return along with
    // the schedule keeping the plan alive.
    unsafe { root.enter_segment(&mut entry) };
    debug_assert!(entry.is_done());

    RawEntityIdAccess {
        cursor: first.cursor(),
        schedule: Some(schedule),
    }
}

// Why the walks yield every match of the query exactly once.
//
// Required members are the storages of the selected cover set, each one held by a plain window
// child of the `Mixed` owning the group. A match owns a component in all of them.
// Other children are looked up, except where a range proves they hold the walked entity.
//
// When that `Mixed` is nested, its ancestors drive through it alone and look the rest of the
// query up. Their matches are its matches that pass these lookups, so the walks below yield
// each of them exactly once too. Lookups don't depend on the segment, a candidate rejected
// in one segment would be rejected in all of them.
//
// Each walk draws its entities from a required member `X`: any required member for an aligned
// walk since they all hold the same entities there, the owner otherwise. The walk meets `e`
// exactly when `e`'s component in `X` is in the region of `X` the walk visits.
// Placement cuts walks into pieces wherever a captain's ring wraps, so that each captain
// advances linearly. Pieces visit the same positions as their walk, the cuts don't change
// which entities are met.
//
// Completeness, for a match `e`:
// - When one of `e`'s required members is unevaluated, that member's unevaluated walk meets `e`.
//   Coverage only rejects it when an earlier walk already met it, and so yielded it.
//   This doesn't depend on the other members' regions: if A was evaluated as ungrouped
//   and `e` gained B afterwards, B's walk finds `e` and looks A up. Owning A doesn't mean
//   one of A's walks met `e`.
// - Otherwise all of `e`'s required components are evaluated. Regroup places an entity in the
//   union of the connected groups it satisfies. `e` satisfies every group of the cover set and
//   they're connected, so `e` is in a range whose group contains all required members, the
//   same range in each of them. Say it's at logical position `p` of the range's damaged owner:
//   - below the aligned length, all required members hold the same entity at `p`:
//     the range's aligned walks yield `e`
//   - from the aligned length on, `p` is in the owner's damaged suffix: the damaged walks yield `e`
//   That's why ranges missing a required member and evaluated ungrouped regions are never
//   walked, they can't hold a match. With AB and AC, an entity holding A, B and C is in ABC.
//
// Uniqueness:
// - A walk visits distinct positions of a single region of `X`, it meets each entity at most once.
// - An aligned walk meets `e` when `e` is at one of its positions in every required member.
//   Every other walk visits other regions of its own `X`: other positions of the range,
//   other ranges or the unevaluated region. None of them meets `e`, aligned walks need no record.
// - Otherwise, a later walk meets `e` again only if `e`'s component in an earlier walk's owner
//   storage lies in the region that walk visited. Later walks get a coverage record for each of
//   these regions and reject their candidates before creating any item.
//   When the earlier owner is one of the later walk's captains, its record is left out: that
//   captain reads `e` in the region the later walk visits, which is distinct from the
//   earlier one. Having several captains doesn't make a damaged walk aligned, it still records
//   what it owns.
//   An earlier candidate a lookup rejected isn't a match, the later walk rejects it too.

/// Builds the plan walking `alternative`'s walks in order, `None` when it has none.
///
/// `root` is the query's root, the `Mixed` that reported `alternative`.
fn grouped_plan<S: Shiperator>(root: &S, alternative: &Alternative) -> Option<Plan> {
    let row = SegmentRow::of(root);
    let layout = row.layout();

    // Every segment drives the same path down to the `Mixed` owning the group,
    // the rest of the query is made of sailors.
    let (base_row, owner_slot, candidate) = route_row(layout, alternative);
    if candidate.walks.is_empty() {
        return None;
    }

    let slots = child_slots(layout, owner_slot);

    for member in &candidate.members {
        // The regions and the entities were both read from the member's storage while it's
        // borrowed. This catches a constructor pairing a child with another child's entities.
        assert!(
            member.entities.local_index() == 0 && member.entities.len() == member.len,
            "a member's entities are its whole storage"
        );
    }

    // Walks are never empty and neither are their pieces, every placement is a segment.
    let placements: Vec<Placement> = candidate
        .walks
        .iter()
        .flat_map(|&walk| Placement::of(walk, candidate))
        .collect();

    let mut segments = Vec::with_capacity(placements.len());
    let mut rows = Vec::with_capacity(placements.len() * layout.len());
    let mut coverage = Vec::new();

    for (index, placement) in placements.iter().enumerate() {
        let records = placement.coverage(&placements[..index]);
        let span = CoverageSpan {
            start: coverage.len(),
            len: records.len(),
        };
        coverage.extend(records);

        // Each segment uses its own row, rows are pushed in segment order.
        rows.extend(placement.row(&base_row, owner_slot, &slots, span));
        segments.push(SegmentDescriptor::from_cursor(placement.entities, index));
    }

    let plan = Plan::new(layout, segments, rows, coverage)
        .expect("grouped plans are built for their query's layout");

    Some(plan)
}

/// A piece of a walk placed in its captains' storages, one segment of the plan.
struct Placement {
    /// The entities the piece visits, from the storage of one of its captains.
    entities: EntityCursor,
    /// Children driving the piece, each with the index in its storage of the piece's first entity.
    captains: Vec<(usize, usize)>,
    /// The region later pieces must not yield again, `None` for aligned walks.
    owned: Option<Coverage>,
}

impl Placement {
    /// Places `walk`, one of `candidate`'s walks, in its captains' storages.
    ///
    /// This is where logical positions become physical indices. A walk crossing the wrap point
    /// of one of its captains' rings is cut there, so it can give several placements.
    fn of(walk: Walk, candidate: &GroupedCandidate) -> Vec<Placement> {
        let placements = match walk {
            Walk::Aligned {
                range,
                start,
                len,
                captains,
            } => {
                // Captains hold the same entities over these positions, any of them can drive.
                // Later walks never meet these entities, see the reasoning above `grouped_plan`.
                let driver = captains.trailing_zeros() as usize;

                Placement::ring(
                    candidate,
                    range,
                    driver,
                    start..start + len,
                    captains,
                    false,
                )
            }
            Walk::Damaged {
                range,
                owner,
                start,
                len,
                captains,
            } => Placement::ring(candidate, range, owner, start..start + len, captains, true),
            // The unevaluated region isn't a ring, its positions are physical indices already.
            Walk::Unevaluated { owner, start, len } => {
                vec![Placement::piece(
                    candidate,
                    owner,
                    vec![(owner, start)],
                    len,
                    true,
                )]
            }
        };

        // Captain windows resolve `segment_start + local index` without bounds checks,
        // every captain's part of a piece has to fit in its storage.
        for placement in &placements {
            for &(child, start) in &placement.captains {
                let end = start.checked_add(placement.entities.len());

                assert!(
                    end.is_some_and(|end| end <= member(candidate, child).len),
                    "a walk fits in its captains' storages"
                );
            }
        }

        placements
    }

    /// Places logical positions `positions` of `candidate`'s range at position `range`,
    /// driven by child `driver` with the children `captains` flags.
    ///
    /// Gives one piece per stretch where every captain's part is physically consecutive.
    /// When `owns` is set, each piece owns `driver`'s part of it.
    fn ring(
        candidate: &GroupedCandidate,
        range: usize,
        driver: usize,
        positions: Range<usize>,
        captains: u32,
        owns: bool,
    ) -> Vec<Placement> {
        let range = &candidate.ranges[range];
        let rings: Vec<(usize, Ring)> = mask_children(captains)
            .map(|child| {
                let member = range
                    .members
                    .iter()
                    .find(|member| member.child == child)
                    .expect("captains are members of their range");

                (child, member.ring)
            })
            .collect();

        let mut placements = Vec::new();
        let mut position = positions.start;
        while position < positions.end {
            // Members' rings wrap at their own points, unrelated to each other's.
            // The piece stops at the first one coming up in any captain.
            // `contiguous_len` is at least 1, so we always make progress.
            let len = rings
                .iter()
                .map(|(_, ring)| ring.contiguous_len(position))
                .fold(positions.end - position, usize::min);
            let starts = rings
                .iter()
                .map(|&(child, ring)| (child, ring.physical_of(position)))
                .collect();

            placements.push(Placement::piece(candidate, driver, starts, len, owns));
            position += len;
        }

        placements
    }

    /// Places `len` entities from `captains`, each one with the index in its storage of the
    /// piece's first entity, driven by child `driver`.
    ///
    /// When `owns` is set, the piece owns `driver`'s part of it.
    fn piece(
        candidate: &GroupedCandidate,
        driver: usize,
        captains: Vec<(usize, usize)>,
        len: usize,
        owns: bool,
    ) -> Placement {
        let driver_start = captains
            .iter()
            .find(|&&(child, _)| child == driver)
            .expect("the driver is a captain")
            .1;
        let driver = member(candidate, driver);
        // `segment` checks the piece is inside the storage, `driver_start + len` can't overflow below.
        let entities = driver.entities.segment(driver_start, len);

        let owned = owns.then(|| Coverage {
            child: u32::try_from(driver.child).expect("a Mixed has at most 32 children"),
            start: driver_start,
            end: driver_start + len,
        });

        Placement {
            entities,
            captains,
            owned,
        }
    }

    /// Returns the regions earlier pieces own that this piece could meet again.
    fn coverage(&self, earlier: &[Placement]) -> Vec<Coverage> {
        // Our candidates are in another region of our own storage, matching them against
        // that storage's earlier regions would never reject anything.
        earlier
            .iter()
            .filter_map(|placement| placement.owned)
            .filter(|region| {
                !self
                    .captains
                    .iter()
                    .any(|&(child, _)| child == region.child as usize)
            })
            .collect()
    }

    /// Returns the row this walk installs.
    ///
    /// `base_row` is the row leading to the `Mixed` owning the group, at position `owner_slot`.
    /// `slots` holds the position of each of its children.
    fn row(
        &self,
        base_row: &[NodeState],
        owner_slot: usize,
        slots: &[usize],
        coverage: CoverageSpan,
    ) -> Vec<NodeState> {
        let captain_mask = self
            .captains
            .iter()
            .fold(0, |mask, &(child, _)| mask | 1 << child);

        // Children not driving the walk are sailors, along with their whole subtree.
        let mut row = base_row.to_vec();
        row[owner_slot] = NodeState::Mixed {
            captain_mask,
            coverage,
        };
        for &(child, segment_start) in &self.captains {
            row[slots[child]] = NodeState::Window { segment_start };
        }

        row
    }
}

/// Returns `candidate`'s member held by the child at position `child`.
fn member(candidate: &GroupedCandidate, child: usize) -> &Member {
    candidate
        .members
        .iter()
        .find(|member| member.child == child)
        .expect("members drive grouped walks")
}

/// Follows `alternative`'s route from `layout`'s root down to the `Mixed` owning its group.
///
/// Returns the row every segment shares, the position of the owner in `layout` and its walks.\
/// In that row, each `Mixed` on the way drives through the child leading to the owner,
/// without coverage. Every other node is a sailor, the owner's state is left to each segment.
fn route_row<'a>(
    layout: &[NodeKind],
    alternative: &'a Alternative,
) -> (Vec<NodeState>, usize, &'a GroupedCandidate) {
    let mut row = vec![NodeState::Sailor; layout.len()];
    let mut slot = 0;
    let mut alternative = alternative;

    loop {
        match &alternative.route {
            Route::Local(candidate) => return (row, slot, candidate),
            Route::Nested {
                child,
                alternative: nested,
            } => {
                // Coverage stays with the owner, the `Mixed`s above it never reject a candidate
                // because another segment walked it.
                row[slot] = NodeState::Mixed {
                    captain_mask: 1 << child,
                    coverage: CoverageSpan::EMPTY,
                };

                slot = child_slots(layout, slot)[*child];
                alternative = nested;
            }
        }
    }
}

/// Returns the position in `layout` of each immediate child of the `Mixed` at position `node`.
fn child_slots(layout: &[NodeKind], node: usize) -> Vec<usize> {
    let NodeKind::Mixed { children } = layout[node] else {
        panic!("grouped walks drive a Mixed");
    };

    let mut slots = Vec::new();
    let mut next = node + 1;
    for _ in 0..children {
        slots.push(next);
        next = subtree_end(layout, next);
    }

    slots
}

/// Returns the position following the subtree rooted at `node` in `layout`.
fn subtree_end(layout: &[NodeKind], node: usize) -> usize {
    let children = match layout[node] {
        NodeKind::Window => 0,
        NodeKind::Or => 2,
        NodeKind::Mixed { children } => children,
    };

    (0..children).fold(node + 1, |next, _| subtree_end(layout, next))
}
