//! Segments of a grouped alternative, over the storages a query borrows.
//!
//! [`grouped_entities`] publishes the walks [`select`](super::select) estimated as a plan:
//! one segment per walk, each with the row it installs and the regions earlier walks own.\
//! It doesn't read entities, segments are carved out of the members' entities from metadata alone.
//!
//! The group can be held by a nested `Mixed`. Each `Mixed` between the root and that one then
//! drives every segment through the single child leading to it, the rest of the query is looked up.

use super::{mask_children, Alternative, GroupedCandidate, Member, NodeMetadata, Route, Walk};
use crate::iter::plan::{
    Coverage, CoverageSpan, NodeKind, NodeState, Plan, SegmentDescriptor, SegmentEntry, SegmentRow,
    SegmentSchedule,
};
use crate::iter::{EntityCursor, RawEntityIdAccess, Shiperator};
use alloc::vec;
use alloc::vec::Vec;

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
    // SAFE: `grouped_plan` built every row for `root`'s own layout. In each segment, a captain
    // window starts at the index, in its own storage, of the segment's first entity and the
    // segment fits in that storage. For the aligned walk, this relies on members holding
    // the same entities over the prefix they all trust, which their group metadata guarantees.
    // The `Mixed`s above the group's owner only have the child leading to it as captain, they
    // pass the candidates down unchanged.
    // `root` gets this segment's candidates from `first`'s cursor, which we return along with
    // the schedule keeping the plan alive.
    unsafe { root.enter_segment(&mut entry) };
    debug_assert!(entry.is_done());

    RawEntityIdAccess {
        cursor: first.cursor(),
        schedule: Some(schedule),
        metadata: NodeMetadata::Ungrouped,
    }
}

// Why the walks yield every match of the query exactly once.
//
// Members are the selected group's storages, each one held by a plain window child of the
// `Mixed` owning the group. A match owns a component in all of them. Other children are only
// looked up.
//
// When that `Mixed` is nested, its ancestors drive through it alone and look the rest of the
// query up. Their matches are its matches that pass these lookups, so the walks below yield
// each of them exactly once too. Lookups don't depend on the segment, a candidate rejected
// in one segment would be rejected in all of them.
//
// Completeness, for a match `e`:
// - When one of `e`'s members is unevaluated, that member's unevaluated walk meets `e`.
//   Coverage only rejects it when an earlier walk already met it, and so yielded it.
//   This doesn't depend on the other members' regions: if A was evaluated as ungrouped
//   and `e` gained B afterwards, B's walk finds `e` and looks A up. Owning A doesn't mean
//   one of A's walks met `e`.
// - Otherwise all of `e`'s members are evaluated. Regroup never leaves components that
//   satisfy a group together in ungrouped regions, so `e` is in every member's group range.
//   Say it's at logical position `p` of the damaged owner's range:
//   - below the aligned length, all members hold the same entity at `p`: the aligned walk yields `e`
//   - from the aligned length on, `p` is in the owner's damaged suffix: the damaged walk yields `e`
//   That's why evaluated ungrouped regions are never walked, they can't hold a match.
//
// Uniqueness:
// - A walk visits a range of a single storage, it meets each entity at most once.
// - The aligned walk's entities have every member in the aligned prefix. Later walks only meet
//   entities whose owner component is outside of it, none of them were yielded there.
// - Otherwise, a later walk meets `e` again only if `e`'s component in an earlier walk's owner
//   storage lies in the region that walk visited. Later walks get a coverage record for each of
//   these regions and reject their candidates before creating any item.
//   Regions in the later walk's own storage are left out, its candidates are in another region.
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

    let placements: Vec<Placement> = candidate
        .walks
        .iter()
        .map(|&walk| Placement::new(walk, candidate))
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

/// A walk placed in its members' storages.
struct Placement {
    /// The entities the walk visits, from the storage of one of its captains.
    entities: EntityCursor,
    /// Children driving the walk, each with the index in its storage of the walk's first entity.
    captains: Vec<(usize, usize)>,
    /// The region later walks must not yield again, `None` for the aligned walk.
    owned: Option<Coverage>,
}

impl Placement {
    /// Places `walk`, one of `candidate`'s walks, in its captains' storages.
    ///
    /// This is where range positions become physical indices.
    fn new(walk: Walk, candidate: &GroupedCandidate) -> Placement {
        let member = |child| {
            candidate
                .members
                .iter()
                .find(|member| member.child == child)
                .expect("members drive grouped walks")
        };

        let placement = match walk {
            Walk::Aligned {
                range,
                start,
                len,
                captains,
            } => {
                // Captains hold the same entities over these positions, any of them can drive.
                let driver = captains.trailing_zeros() as usize;
                let placement = Placement::range(candidate, range, driver, start, len, captains);

                Placement {
                    // Later walks never meet these entities, see the reasoning above `grouped_plan`.
                    owned: None,
                    ..placement
                }
            }
            Walk::Damaged {
                range,
                owner,
                start,
                len,
                captains,
            } => Placement::range(candidate, range, owner, start, len, captains),
            Walk::Unevaluated { owner, start, len } => Placement::single(member(owner), start, len),
        };

        // Captain windows resolve `segment_start + local index` without bounds checks,
        // every captain's part of the walk has to fit in its storage.
        for &(child, start) in &placement.captains {
            let end = start.checked_add(walk.len());

            assert!(
                end.is_some_and(|end| end <= member(child).len),
                "a walk fits in its captains' storages"
            );
        }

        placement
    }

    /// Places logical positions `start..start + len` of `candidate`'s range at position `range`,
    /// driven by child `driver` with the children `captains` flags.
    ///
    /// The walk owns `driver`'s part of the range.
    fn range(
        candidate: &GroupedCandidate,
        range: usize,
        driver: usize,
        start: usize,
        len: usize,
        captains: u32,
    ) -> Placement {
        let range = &candidate.ranges[range];

        // `is_placeable` only lets unrotated rings through, so logical position `p` is at
        // physical index `ring.start + p` and each walk is a single physical piece in every
        // captain. Rings will split the walks crossing their wrap point here.
        let captains: Vec<(usize, usize)> = mask_children(captains)
            .map(|child| {
                let ring = range
                    .members
                    .iter()
                    .find(|member| member.child == child)
                    .expect("captains are members of their range")
                    .ring;
                assert!(
                    ring.contiguous_len(start) >= len,
                    "walks don't cross a wrap point"
                );

                (child, ring.physical_of(start))
            })
            .collect();

        let driver_start = captains
            .iter()
            .find(|&&(child, _)| child == driver)
            .expect("the driver is a captain")
            .1;
        let driver = candidate
            .members
            .iter()
            .find(|member| member.child == driver)
            .expect("members drive grouped walks");
        // `segment` checks the walk is inside the storage, `driver_start + len` can't overflow below.
        let entities = driver.entities.segment(driver_start, len);

        Placement {
            entities,
            captains,
            owned: Some(Coverage {
                child: u32::try_from(driver.child).expect("a Mixed has at most 32 children"),
                start: driver_start,
                end: driver_start + len,
            }),
        }
    }

    /// Places a walk driven by `owner` alone, over `len` entities from physical index `start`.
    fn single(owner: &Member, start: usize, len: usize) -> Placement {
        // `segment` checks the walk is inside the storage, `start + len` can't overflow below.
        let entities = owner.entities.segment(start, len);

        Placement {
            entities,
            captains: vec![(owner.child, start)],
            owned: Some(Coverage {
                child: u32::try_from(owner.child).expect("a Mixed has at most 32 children"),
                start,
                end: start + len,
            }),
        }
    }

    /// Returns the regions earlier walks own that this walk could meet again.
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
