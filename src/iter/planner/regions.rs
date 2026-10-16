//! Segments of a grouped alternative, over the storages a query borrows.
//!
//! [`grouped_entities`] publishes the walks [`select`](super::select) estimated as a plan:
//! one segment per walk, each with the row it installs and the regions earlier walks own.\
//! It doesn't read entities, segments are carved out of the members' entities from metadata alone.

use super::{GroupedCandidate, Member, NodeMetadata, Walk};
use crate::iter::plan::{
    Coverage, CoverageSpan, NodeKind, NodeState, Plan, SegmentDescriptor, SegmentEntry, SegmentRow,
    SegmentSchedule,
};
use crate::iter::{EntityCursor, RawEntityIdAccess, Shiperator};
use alloc::vec;
use alloc::vec::Vec;

/// Returns the entities driving `mixed` through every walk of `candidate`, with `mixed`
/// installed for the first one.
///
/// `mixed` is the `Mixed` whose immediate children `candidate` was selected from,
/// in the state its constructor built.\
/// `mixed`'s new state points into the plan held by the returned entities, they have to drive it.
///
/// When `candidate` has no walk, the returned entities have no candidate and no plan,
/// `mixed` is left as is.
pub(crate) fn grouped_entities<S: Shiperator>(
    mixed: &mut S,
    candidate: &GroupedCandidate,
) -> RawEntityIdAccess {
    let Some(plan) = grouped_plan(mixed, candidate) else {
        return RawEntityIdAccess::empty();
    };

    let mut schedule = SegmentSchedule::new(plan);
    let first = schedule
        .take_next()
        .expect("plans have at least one segment");

    // The first segment is entered like any follow-up, from its row in the published plan.
    let mut entry = SegmentEntry::new(schedule.plan(), &first);
    // SAFE: `grouped_plan` built every row for `mixed`'s own layout. In each segment, a captain
    // window starts at the index, in its own storage, of the segment's first entity and the
    // segment fits in that storage. For the aligned walk, this relies on members holding
    // the same entities over the prefix they all trust, which their group metadata guarantees.
    // `mixed` gets this segment's candidates from `first`'s cursor, which we return along with
    // the schedule keeping the plan alive.
    unsafe { mixed.enter_segment(&mut entry) };
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
// query. A match owns a component in all of them. Other children are only looked up.
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

/// Builds the plan walking `candidate`'s walks in order, `None` when it has none.
fn grouped_plan<S: Shiperator>(mixed: &S, candidate: &GroupedCandidate) -> Option<Plan> {
    if candidate.walks.is_empty() {
        return None;
    }

    let row = SegmentRow::of(mixed);
    let layout = row.layout();
    let slots = child_slots(layout);

    for member in &candidate.members {
        // The regions and the entities were both read from the member's storage while it's
        // borrowed. This catches a constructor pairing a child with another child's entities.
        assert!(
            member.entities.local_index() == 0 && member.entities.len() == member.regions.len,
            "a member's entities are its whole storage"
        );
    }

    let placements: Vec<Placement> = candidate
        .walks
        .iter()
        .map(|&walk| Placement::new(walk, &candidate.members))
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
        rows.extend(placement.row(layout, &slots, span));
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
    /// Places `walk` in the storages of `members`.
    ///
    /// This is where range positions become physical indices.
    fn new(walk: Walk, members: &[Member]) -> Placement {
        let member = |child| {
            members
                .iter()
                .find(|member| member.child == child)
                .expect("members drive grouped walks")
        };

        // Ranges aren't rotated, `NodeMetadata::of_storage` rejects a nonzero shift. So logical
        // position `p` of a range is at physical index `range_start + p` and each walk is
        // a single physical piece. Rings will split the walks crossing their wrap point here.
        let placement = match walk {
            Walk::Aligned { len } => {
                // Members hold the same entities over the aligned prefix, the first member's
                // entities drive the walk and every member starts at its own range's start.
                let driver = &members[0];

                Placement {
                    entities: driver.entities.segment(driver.regions.range_start, len),
                    captains: members
                        .iter()
                        .map(|member| (member.child, member.regions.range_start))
                        .collect(),
                    // Later walks never meet these entities, see the reasoning above `grouped_plan`.
                    owned: None,
                }
            }
            Walk::Damaged { owner, start, len } => {
                let owner = member(owner);

                Placement::single(owner, owner.regions.range_start + start, len)
            }
            Walk::Unevaluated { owner, start, len } => Placement::single(member(owner), start, len),
        };

        // Captain windows resolve `segment_start + local index` without bounds checks,
        // every captain's part of the walk has to fit in its storage.
        for &(child, start) in &placement.captains {
            let end = start.checked_add(walk.len());

            assert!(
                end.is_some_and(|end| end <= member(child).regions.len),
                "a walk fits in its captains' storages"
            );
        }

        placement
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

    /// Returns the row this walk installs, in `layout`'s preorder.
    ///
    /// `slots` holds the position in `layout` of each child of the root.
    fn row(&self, layout: &[NodeKind], slots: &[usize], coverage: CoverageSpan) -> Vec<NodeState> {
        let captain_mask = self
            .captains
            .iter()
            .fold(0, |mask, &(child, _)| mask | 1 << child);

        // Children not driving the walk are sailors, along with their whole subtree.
        let mut row = vec![NodeState::Sailor; layout.len()];
        row[0] = NodeState::Mixed {
            captain_mask,
            coverage,
        };
        for &(child, segment_start) in &self.captains {
            row[slots[child]] = NodeState::Window { segment_start };
        }

        row
    }
}

/// Returns the position of each immediate child of `layout`'s root, a `Mixed`, in `layout`.
fn child_slots(layout: &[NodeKind]) -> Vec<usize> {
    let NodeKind::Mixed { children } = layout[0] else {
        panic!("grouped walks drive a Mixed");
    };

    let mut slots = Vec::new();
    let mut next = 1;
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
