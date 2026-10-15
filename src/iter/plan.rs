//! Shared immutable segment plans.
//!
//! A [`Plan`] describes a traversal split into segments: the entities each segment walks,
//! the state every typed node has to install when entering it and the regions earlier
//! segments already own.\
//! Once published, a plan never changes. [`SegmentSchedule`]s share it and each tracks
//! its own position in the follow-up segments, independently of the active `EntityCursor`.

use crate::entity_id::EntityId;
use crate::sparse_set::EntityCursor;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::ManuallyDrop;
use core::ptr::NonNull;

/// Kind of a typed query node.
///
/// A query's layout lists its nodes in preorder. It is used to validate plan rows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "grouping plan 04 wires plans into segment entry")
)]
pub(crate) enum NodeKind {
    /// A node without children, like a component window or an entity slice.
    ///
    /// Wrappers forwarding to a single window (tracking, `Not`, ...) are part of this node.
    Window,
    /// A `Mixed` with `children` immediate children, between 1 and 32.
    Mixed { children: u32 },
    /// An `Or` with its two branches.
    Or,
}

/// Branch of an `Or` acting as captain.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "grouping plan 04 wires plans into segment entry")
)]
pub(crate) enum OrBranch {
    A,
    B,
}

/// State a node installs when entering a segment.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "grouping plan 04 wires plans into segment entry")
)]
pub(crate) enum NodeState {
    /// The node's subtree isn't driving this segment, it only answers sailor lookups.
    ///
    /// Every node of a sailor subtree has this state.
    Sailor,
    /// A captain window and the absolute index of its segment's first component.
    Window { segment_start: usize },
    /// A captain `Mixed`.
    ///
    /// `captain_mask` flags its immediate children driving the segment, nested `Mixed`s have their own.\
    /// `coverage` lists the regions earlier segments own in its children's storages.
    Mixed {
        captain_mask: u32,
        coverage: CoverageSpan,
    },
    /// A captain `Or` and the branch driving the segment.
    Or { branch: OrBranch },
}

/// Range of records in a plan's coverage pool.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct CoverageSpan {
    pub(crate) start: usize,
    pub(crate) len: usize,
}

/// Physical region of a storage owned by an earlier segment.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Coverage {
    /// Immediate child of the `Mixed` holding this record, whose storage contains the region.
    pub(crate) child: u32,
    /// Absolute index of the region's first component.
    pub(crate) start: usize,
    /// Absolute index one past the region's last component.
    pub(crate) end: usize,
}

/// Entities walked by a segment and the location of its node-state row.
#[derive(Clone, Copy)]
pub(crate) struct SegmentDescriptor {
    entities: NonNull<EntityId>,
    len: usize,
    row: usize,
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "grouping plan 04 wires plans into segment entry")
)]
impl SegmentDescriptor {
    /// Returns a descriptor over the `len` entities starting at `entities`, using row `row`.
    ///
    /// Plans don't own the ECS storages, the entities stay borrowed from them.
    ///
    /// ### Safety
    ///
    /// When `len` isn't 0:
    /// - `entities` must point to `len` consecutive initialized entities within a single allocation
    /// - these entities must not be deallocated or modified while this descriptor, a copy of it,
    ///   a plan holding it or a cursor obtained from any of them is used
    #[inline]
    pub(crate) unsafe fn new(entities: NonNull<EntityId>, len: usize, row: usize) -> Self {
        SegmentDescriptor { entities, len, row }
    }

    /// Returns the number of candidates of the segment.
    ///
    /// Candidates aren't results, a candidate can fail its lookups.
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Returns a cursor over the segment's entities.
    #[inline]
    pub(crate) fn cursor(&self) -> EntityCursor {
        // SAFE: `new`'s caller guarantees the entities are valid and stay unmodified
        // while this descriptor and the cursors obtained from it are used.
        unsafe { EntityCursor::new(self.entities, self.len) }
    }
}

/// Structural defect found when building a [`Plan`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PlanError {
    /// The plan has no segment. An empty traversal has no plan at all.
    NoSegment,
    /// A segment has no candidate.
    EmptySegment,
    /// The total number of candidates doesn't fit in a `usize`.
    LengthOverflow,
    /// The layout isn't a single preorder tree or a `Mixed` has an invalid number of children.
    MalformedLayout,
    /// The row pool isn't a whole number of rows.
    MalformedRows,
    /// A segment refers to a row outside the pool.
    RowOutOfBounds,
    /// A state doesn't match its node's kind or activity.
    StateMismatch,
    /// A `Mixed` captain mask is empty or flags a child it doesn't have.
    InvalidCaptainMask,
    /// A coverage span is outside the pool.
    CoverageOutOfBounds,
    /// A coverage record has an empty region or names a child its `Mixed` doesn't have.
    InvalidCoverage,
}

/// Shared immutable description of a segmented traversal.
///
/// Rows hold one state per layout node in preorder.\
/// Validation only covers the plan's structure. It doesn't scan entities, check that they match
/// the query or that mutable outputs are unique, these belong to the planner and to resolution.
///
/// The plan doesn't own the storages its segments walk.
pub(crate) struct Plan {
    segments: Box<[SegmentDescriptor]>,
    rows: Box<[NodeState]>,
    row_width: usize,
    coverage: Box<[Coverage]>,
    /// Number of candidates across all segments.
    total_len: usize,
}

// Plans only point to entities, never to components.
// `SegmentDescriptor::new`'s caller guarantees these entities aren't modified while the plan is used
// and the plan only reads them. `EntityId` is plain data, reading it from any thread is fine.
// Mutable component access stays in the typed windows and their own `Send` bounds,
// sharing a plan doesn't give another way to reach overlapping mutable outputs.
unsafe impl Send for Plan {}
unsafe impl Sync for Plan {}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "grouping plan 04 wires plans into segment entry")
)]
impl Plan {
    /// Builds a plan after checking its structure against `layout`.
    ///
    /// Segments run in order, each uses the row at its row index.\
    /// `rows` concatenates rows of `layout.len()` states, rows can be shared between segments.
    ///
    /// The builder is expected to omit empty segments, a plan without any segment is rejected.
    pub(crate) fn new(
        layout: &[NodeKind],
        segments: Vec<SegmentDescriptor>,
        rows: Vec<NodeState>,
        coverage: Vec<Coverage>,
    ) -> Result<Plan, PlanError> {
        if segments.is_empty() {
            return Err(PlanError::NoSegment);
        }

        let row_width = layout.len();
        if row_width == 0 {
            return Err(PlanError::MalformedLayout);
        }
        if !rows.len().is_multiple_of(row_width) {
            return Err(PlanError::MalformedRows);
        }

        if coverage.iter().any(|record| record.start >= record.end) {
            return Err(PlanError::InvalidCoverage);
        }

        // The root drives every segment so it's always active.
        for row in rows.chunks_exact(row_width) {
            let end = validate_subtree(layout, row, &coverage, 0, true)?;

            // The root's subtree has to span the whole layout, otherwise it's a forest.
            if end != row_width {
                return Err(PlanError::MalformedLayout);
            }
        }

        let row_count = rows.len() / row_width;
        for segment in &segments {
            if segment.len == 0 {
                return Err(PlanError::EmptySegment);
            }
            if segment.row >= row_count {
                return Err(PlanError::RowOutOfBounds);
            }
        }

        let total_len = checked_total_len(segments.iter().map(SegmentDescriptor::len))?;

        Ok(Plan {
            segments: segments.into_boxed_slice(),
            rows: rows.into_boxed_slice(),
            row_width,
            coverage: coverage.into_boxed_slice(),
            total_len,
        })
    }

    /// Returns the node states `segment` installs, in layout preorder.
    ///
    /// `segment` must come from this plan.
    #[inline]
    pub(crate) fn row(&self, segment: &SegmentDescriptor) -> &[NodeState] {
        let start = segment.row * self.row_width;

        &self.rows[start..start + self.row_width]
    }

    /// Returns the coverage records of `span`.
    ///
    /// `span` must come from one of this plan's rows.\
    /// The records don't move while the plan is alive.
    #[inline]
    pub(crate) fn coverage(&self, span: CoverageSpan) -> &[Coverage] {
        coverage_records(&self.coverage, span).expect("coverage span validated by Plan::new")
    }
}

/// Returns the records of `span`, `None` when it's outside `coverage`.
fn coverage_records(coverage: &[Coverage], span: CoverageSpan) -> Option<&[Coverage]> {
    let end = span.start.checked_add(span.len)?;

    coverage.get(span.start..end)
}

/// Sums segment lengths, failing instead of overflowing.
fn checked_total_len(lens: impl IntoIterator<Item = usize>) -> Result<usize, PlanError> {
    lens.into_iter()
        .try_fold(0usize, usize::checked_add)
        .ok_or(PlanError::LengthOverflow)
}

/// Validates the states of the subtree rooted at `node`.
///
/// `active` tells if this node drives the segment. Inactive nodes are sailors and so are all
/// their descendants.\
/// Returns the position following the subtree.
fn validate_subtree(
    layout: &[NodeKind],
    row: &[NodeState],
    coverage: &[Coverage],
    node: usize,
    active: bool,
) -> Result<usize, PlanError> {
    let Some(&kind) = layout.get(node) else {
        // A parent claims more children than the layout has left.
        return Err(PlanError::MalformedLayout);
    };

    let arity = match kind {
        NodeKind::Window => 0,
        NodeKind::Or => 2,
        NodeKind::Mixed { children } if (1..=u32::BITS).contains(&children) => children,
        NodeKind::Mixed { .. } => return Err(PlanError::MalformedLayout),
    };

    // Rows have one state per layout node so `node` is also in bounds here.
    let state = row[node];

    // Bit `i` flags child `i` as active. The children of a sailor are all sailors.
    let active_children = if active {
        active_children(kind, state, coverage)?
    } else if state == NodeState::Sailor {
        0
    } else {
        return Err(PlanError::StateMismatch);
    };

    let mut next = node + 1;
    for child in 0..arity {
        let child_active = active_children & (1 << child) != 0;
        next = validate_subtree(layout, row, coverage, next, child_active)?;
    }

    Ok(next)
}

/// Validates the state of an active node and returns which of its children are active.
fn active_children(
    kind: NodeKind,
    state: NodeState,
    coverage: &[Coverage],
) -> Result<u32, PlanError> {
    match (kind, state) {
        (NodeKind::Window, NodeState::Window { .. }) => Ok(0),
        (NodeKind::Or, NodeState::Or { branch }) => match branch {
            OrBranch::A => Ok(0b01),
            OrBranch::B => Ok(0b10),
        },
        (
            NodeKind::Mixed { children },
            NodeState::Mixed {
                captain_mask,
                coverage: span,
            },
        ) => {
            // We go through u64 so a 32 children shift stays defined.
            if captain_mask == 0 || u64::from(captain_mask) >> children != 0 {
                return Err(PlanError::InvalidCaptainMask);
            }

            let records = coverage_records(coverage, span).ok_or(PlanError::CoverageOutOfBounds)?;
            if records.iter().any(|record| record.child >= children) {
                return Err(PlanError::InvalidCoverage);
            }

            Ok(captain_mask)
        }
        _ => Err(PlanError::StateMismatch),
    }
}

/// Shared ownership of a [`Plan`] and the position of the next follow-up segment.
///
/// Each schedule advances independently, the plan itself is never modified.\
/// A schedule without follow-ups still keeps the plan alive, the current segment might
/// be reading its coverage.
pub(crate) struct SegmentSchedule {
    /// Only taken by `Drop`.
    ///
    /// Releasing the Arc by value kept the iterator's state out of memory in the exploration,
    /// in-place drop glue made consumers copy and reload it. This is a codegen choice,
    /// a plain `Arc` field would be just as correct.
    plan: ManuallyDrop<Arc<Plan>>,
    /// Index of the next follow-up segment.
    next: usize,
    /// Number of candidates in the follow-up segments.
    remaining_len: usize,
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "grouping plan 04 wires plans into segment entry")
)]
impl SegmentSchedule {
    /// Publishes `plan`, all its segments are follow-ups.
    #[inline]
    pub(crate) fn new(plan: Plan) -> SegmentSchedule {
        let remaining_len = plan.total_len;

        SegmentSchedule {
            plan: ManuallyDrop::new(Arc::new(plan)),
            next: 0,
            remaining_len,
        }
    }

    /// Returns the shared plan.
    #[inline]
    pub(crate) fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Returns the next follow-up segment without consuming it.
    #[inline]
    pub(crate) fn peek(&self) -> Option<SegmentDescriptor> {
        self.plan.segments.get(self.next).copied()
    }

    /// Consumes and returns the next follow-up segment.
    #[inline]
    pub(crate) fn take_next(&mut self) -> Option<SegmentDescriptor> {
        let segment = self.peek()?;

        self.next += 1;
        // `remaining_len` is the sum of the lengths of the segments from the old `self.next`,
        // `segment` is one of them so this can't underflow.
        self.remaining_len -= segment.len;

        Some(segment)
    }

    /// Returns the number of follow-up segments left.
    #[inline]
    pub(crate) fn follow_up_count(&self) -> usize {
        self.plan.segments.len() - self.next
    }

    /// Returns the number of candidates in the follow-up segments.
    ///
    /// This is an upper bound of the results they can produce, not an exact count.
    #[inline]
    pub(crate) fn remaining_len(&self) -> usize {
        self.remaining_len
    }

    /// Moves all follow-ups to a new schedule.
    ///
    /// `self` keeps the plan without any follow-up left.
    #[inline]
    pub(crate) fn split_follow_ups(&mut self) -> SegmentSchedule {
        let follow_ups = self.clone();

        self.next = self.plan.segments.len();
        self.remaining_len = 0;

        follow_ups
    }
}

impl Clone for SegmentSchedule {
    #[inline]
    fn clone(&self) -> Self {
        SegmentSchedule {
            plan: ManuallyDrop::new(Arc::clone(&self.plan)),
            next: self.next,
            remaining_len: self.remaining_len,
        }
    }
}

impl Drop for SegmentSchedule {
    #[inline]
    fn drop(&mut self) {
        // SAFE: `plan` is initialized by every constructor and only taken here.
        // `drop` runs once and nothing reads `self` afterwards.
        let plan = unsafe { ManuallyDrop::take(&mut self.plan) };

        drop(plan);
    }
}

#[cfg(test)]
mod grouping_plan_owner {
    use super::*;
    use alloc::vec;
    use core::ptr;

    fn entities(len: u64) -> Vec<EntityId> {
        (0..len).map(EntityId::new).collect()
    }

    fn segment(entities: &[EntityId], row: usize) -> SegmentDescriptor {
        // SAFE: Every test keeps `entities` alive and unmodified until its plans and cursors are gone.
        unsafe { SegmentDescriptor::new(NonNull::from(entities).cast(), entities.len(), row) }
    }

    fn empty_segment(row: usize) -> SegmentDescriptor {
        // SAFE: `len` is 0 so the pointer is never read.
        unsafe { SegmentDescriptor::new(NonNull::dangling(), 0, row) }
    }

    fn window(segment_start: usize) -> NodeState {
        NodeState::Window { segment_start }
    }

    fn mixed(captain_mask: u32, start: usize, len: usize) -> NodeState {
        NodeState::Mixed {
            captain_mask,
            coverage: CoverageSpan { start, len },
        }
    }

    /// A single window plan with one row per segment.
    fn window_plan(segments: &[&[EntityId]]) -> Plan {
        let descriptors = segments
            .iter()
            .enumerate()
            .map(|(row, entities)| segment(entities, row))
            .collect();
        let rows = (0..segments.len()).map(|row| window(row * 10)).collect();

        Plan::new(&[NodeKind::Window], descriptors, rows, Vec::new()).unwrap()
    }

    const MIXED_LAYOUT: [NodeKind; 3] = [
        NodeKind::Mixed { children: 2 },
        NodeKind::Window,
        NodeKind::Window,
    ];

    /// Builds a `Mixed` plan with `rows` and `coverage`, using row 0 for a single segment.
    fn mixed_plan(rows: Vec<NodeState>, coverage: Vec<Coverage>) -> Result<Plan, PlanError> {
        let entities = entities(1);

        Plan::new(&MIXED_LAYOUT, vec![segment(&entities, 0)], rows, coverage)
    }

    fn collect(mut cursor: EntityCursor) -> Vec<EntityId> {
        let mut result = Vec::new();
        while !cursor.is_empty() {
            // SAFE: The cursor isn't empty.
            result.push(unsafe { cursor.current() });
            cursor.advance();
        }

        result
    }

    fn assert_send_sync<T: Send + Sync>() {}

    /// Asserts that schedules and their plans can be sent and shared across threads.
    #[test]
    fn send_sync() {
        assert_send_sync::<Plan>();
        assert_send_sync::<SegmentSchedule>();
    }

    /// Asserts that a plan without any segment is rejected, an empty traversal has no plan.
    #[test]
    fn no_segment() {
        let result = Plan::new(&[NodeKind::Window], Vec::new(), vec![window(0)], Vec::new());

        assert_eq!(result.err(), Some(PlanError::NoSegment));
    }

    /// Asserts that a one segment plan yields its segment once, then keeps the plan
    /// with no follow-up left.
    #[test]
    fn one_segment() {
        let entities = entities(3);
        let mut schedule = SegmentSchedule::new(window_plan(&[&entities]));

        assert_eq!(
            (schedule.follow_up_count(), schedule.remaining_len()),
            (1, 3)
        );

        let segment = schedule.take_next().unwrap();

        assert_eq!(collect(segment.cursor()), entities);
        assert_eq!(schedule.plan().row(&segment), [window(0)]);
        assert_eq!(
            (schedule.follow_up_count(), schedule.remaining_len()),
            (0, 0)
        );
        assert!(schedule.peek().is_none());
    }

    /// Asserts that follow-ups come out in order with their own rows
    /// and that the remaining candidate count shrinks with each of them.
    #[test]
    fn follow_ups() {
        let first = entities(2);
        let second = entities(5);
        let third = entities(1);
        let mut schedule = SegmentSchedule::new(window_plan(&[&first, &second, &third]));

        assert_eq!(schedule.remaining_len(), 8);

        for (entities, remaining_len) in [(&first, 6), (&second, 1), (&third, 0)] {
            let peeked = schedule.peek().unwrap();
            let segment = schedule.take_next().unwrap();

            assert_eq!(peeked.entities, segment.entities);
            assert_eq!(collect(segment.cursor()), *entities);
            assert_eq!(schedule.remaining_len(), remaining_len);
        }

        let rows: Vec<_> = schedule
            .plan
            .segments
            .iter()
            .map(|s| schedule.plan().row(s)[0])
            .collect();
        assert_eq!(rows, [window(0), window(10), window(20)]);
    }

    /// Asserts that an exhausted schedule keeps returning no segment without changing its counts.
    #[test]
    fn exhausted() {
        let entities = entities(2);
        let mut schedule = SegmentSchedule::new(window_plan(&[&entities, &entities]));
        schedule.take_next();
        schedule.take_next();

        for _ in 0..3 {
            assert!(schedule.take_next().is_none());
            assert_eq!(
                (schedule.follow_up_count(), schedule.remaining_len()),
                (0, 0)
            );
        }
    }

    /// Asserts that rows can be shared by several segments.
    #[test]
    fn shared_row() {
        let entities = entities(2);
        let plan = Plan::new(
            &[NodeKind::Window],
            vec![segment(&entities, 0), segment(&entities, 0)],
            vec![window(4)],
            Vec::new(),
        )
        .unwrap();
        let mut schedule = SegmentSchedule::new(plan);

        while let Some(segment) = schedule.take_next() {
            assert_eq!(schedule.plan().row(&segment), [window(4)]);
        }
    }

    /// Asserts that advancing a clone doesn't move the original and that both share one plan.
    #[test]
    fn clone_positions() {
        let first = entities(2);
        let second = entities(3);
        let mut original = SegmentSchedule::new(window_plan(&[&first, &second]));
        original.take_next();

        let mut clone = original.clone();
        clone.take_next();

        assert!(ptr::eq(original.plan(), clone.plan()));
        assert_eq!(
            (original.follow_up_count(), original.remaining_len()),
            (1, 3)
        );
        assert_eq!((clone.follow_up_count(), clone.remaining_len()), (0, 0));
        assert_eq!(collect(original.take_next().unwrap().cursor()), second);
    }

    /// Asserts that splitting gives every follow-up to the new schedule
    /// and leaves the original with the plan but no follow-up.
    #[test]
    fn split_follow_ups() {
        let first = entities(2);
        let second = entities(3);
        let mut left = SegmentSchedule::new(window_plan(&[&first, &second]));
        left.take_next();

        let mut right = left.split_follow_ups();

        assert!(ptr::eq(left.plan(), right.plan()));
        assert_eq!((left.follow_up_count(), left.remaining_len()), (0, 0));
        assert!(left.take_next().is_none());
        assert_eq!((right.follow_up_count(), right.remaining_len()), (1, 3));
        assert_eq!(collect(right.take_next().unwrap().cursor()), second);
    }

    /// Asserts that the plan is destroyed exactly once, when its last owner is dropped.
    #[test]
    fn destroy_once() {
        let entities = entities(2);
        let mut schedule = SegmentSchedule::new(window_plan(&[&entities]));
        let weak = Arc::downgrade(&schedule.plan);
        let clone = schedule.clone();
        let split = schedule.split_follow_ups();

        assert_eq!(weak.strong_count(), 3);

        drop(split);
        assert_eq!(weak.strong_count(), 2);

        drop(schedule);
        assert_eq!(weak.strong_count(), 1);

        drop(clone);
        assert!(weak.upgrade().is_none());
    }

    const EXPECTED_COVERAGE: [Coverage; 1] = [Coverage {
        child: 0,
        start: 0,
        end: 4,
    }];

    /// Builds a schedule over three segments, the last two reading one coverage record.
    ///
    /// The schedule has entered the second segment and has one follow-up left.    /// Returns it with the coverage of its current segment, as the raw slice a `Mixed`
    /// caches when entering a segment.
    fn coverage_schedule(
        first: &[EntityId],
        second: &[EntityId],
    ) -> (SegmentSchedule, *const [Coverage]) {
        let rows = vec![
            mixed(0b01, 0, 0),
            window(0),
            NodeState::Sailor,
            mixed(0b10, 0, 1),
            NodeState::Sailor,
            window(7),
        ];
        let segments = vec![segment(first, 0), segment(second, 1), segment(second, 1)];
        let plan = Plan::new(&MIXED_LAYOUT, segments, rows, EXPECTED_COVERAGE.to_vec()).unwrap();

        let mut schedule = SegmentSchedule::new(plan);
        schedule.take_next();
        let current = schedule.take_next().unwrap();

        let NodeState::Mixed { coverage, .. } = schedule.plan().row(&current)[0] else {
            unreachable!()
        };
        let coverage: *const [Coverage] = schedule.plan().coverage(coverage);

        (schedule, coverage)
    }

    /// Asserts that after a split, the half without follow-ups keeps its current coverage
    /// readable once the half holding the follow-ups is dropped.
    #[test]
    fn coverage_right_dropped() {
        let first = entities(1);
        let second = entities(2);
        let (mut left, coverage) = coverage_schedule(&first, &second);
        let right = left.split_follow_ups();
        drop(right);

        assert_eq!(left.follow_up_count(), 0);
        // SAFE: `left` still owns the plan holding these records.
        assert_eq!(unsafe { &*coverage }, EXPECTED_COVERAGE);
    }

    /// Asserts that after a split, the half holding the follow-ups keeps the shared current
    /// coverage readable once the half without follow-ups is dropped.
    #[test]
    fn coverage_left_dropped() {
        let first = entities(1);
        let second = entities(2);
        let (mut left, coverage) = coverage_schedule(&first, &second);
        let right = left.split_follow_ups();
        drop(left);

        assert_eq!(right.follow_up_count(), 1);
        // SAFE: `right` still owns the plan holding these records.
        assert_eq!(unsafe { &*coverage }, EXPECTED_COVERAGE);
    }

    /// Asserts that schedules dropped while unwinding release the plan exactly once.
    #[cfg(feature = "std")]
    #[test]
    fn unwind() {
        let entities = entities(2);
        let schedule = SegmentSchedule::new(window_plan(&[&entities]));
        let weak = Arc::downgrade(&schedule.plan);

        let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(move || {
            let mut schedule = schedule;
            let _clone = schedule.clone();
            let _split = schedule.split_follow_ups();

            panic!("unwinding with three schedules alive");
        }));

        assert!(result.is_err());
        assert!(weak.upgrade().is_none());
    }

    /// Asserts that validation doesn't promise entity matching or unique outputs:
    /// two segments walking the same entities with the same window start are accepted.
    #[test]
    fn no_uniqueness_promise() {
        let entities = entities(2);
        let plan = Plan::new(
            &[NodeKind::Window],
            vec![segment(&entities, 0), segment(&entities, 0)],
            vec![window(0)],
            Vec::new(),
        );

        assert!(plan.is_ok());
    }

    /// Asserts that a segment without candidates is rejected.
    #[test]
    fn zero_length() {
        let entities = entities(2);
        let result = Plan::new(
            &[NodeKind::Window],
            vec![segment(&entities, 0), empty_segment(0)],
            vec![window(0)],
            Vec::new(),
        );

        assert_eq!(result.err(), Some(PlanError::EmptySegment));
    }

    /// Asserts that a segment referring to a row past the pool is rejected,
    /// including a row index whose offset would overflow.
    #[test]
    fn row_bounds() {
        let entities = entities(2);

        for row in [1, usize::MAX] {
            let result = Plan::new(
                &[NodeKind::Window],
                vec![segment(&entities, row)],
                vec![window(0)],
                Vec::new(),
            );

            assert_eq!(result.err(), Some(PlanError::RowOutOfBounds));
        }
    }

    /// Asserts that a row pool holding a partial row is rejected.
    #[test]
    fn partial_row() {
        let rows = vec![mixed(0b01, 0, 0), window(0), NodeState::Sailor, window(0)];

        assert_eq!(
            mixed_plan(rows, Vec::new()).err(),
            Some(PlanError::MalformedRows)
        );
    }

    /// Asserts that coverage spans past the pool are rejected, including one whose end overflows.
    #[test]
    fn coverage_bounds() {
        let record = Coverage {
            child: 0,
            start: 0,
            end: 1,
        };

        for (start, len) in [(0, 2), (1, 1), (usize::MAX, 2)] {
            let rows = vec![mixed(0b01, start, len), window(0), NodeState::Sailor];

            assert_eq!(
                mixed_plan(rows, vec![record]).err(),
                Some(PlanError::CoverageOutOfBounds)
            );
        }
    }

    /// Asserts that coverage records with an empty region or naming a missing child are rejected.
    #[test]
    fn coverage_records() {
        let records = [
            Coverage {
                child: 0,
                start: 3,
                end: 3,
            },
            Coverage {
                child: 2,
                start: 0,
                end: 1,
            },
        ];

        for record in records {
            let rows = vec![mixed(0b01, 0, 1), window(0), NodeState::Sailor];

            assert_eq!(
                mixed_plan(rows, vec![record]).err(),
                Some(PlanError::InvalidCoverage)
            );
        }
    }

    /// Asserts that the total candidate count overflowing a `usize` is rejected.
    #[test]
    fn length_overflow() {
        assert_eq!(
            checked_total_len([usize::MAX, 1]),
            Err(PlanError::LengthOverflow)
        );
        assert_eq!(checked_total_len([usize::MAX - 1, 1]), Ok(usize::MAX));
    }

    /// Asserts that a state of the wrong kind is rejected, including a sailor root.
    #[test]
    fn state_tag() {
        let rows = [
            [window(0), window(0), NodeState::Sailor],
            [NodeState::Sailor, NodeState::Sailor, NodeState::Sailor],
            [mixed(0b01, 0, 0), mixed(0b01, 0, 0), NodeState::Sailor],
        ];

        for row in rows {
            assert_eq!(
                mixed_plan(row.to_vec(), Vec::new()).err(),
                Some(PlanError::StateMismatch)
            );
        }
    }

    /// Asserts that a sailor child with an active state is rejected,
    /// as are the descendants of a sailor `Or` branch.
    #[test]
    fn sailor_subtree() {
        let rows = vec![mixed(0b01, 0, 0), window(0), window(0)];
        assert_eq!(
            mixed_plan(rows, Vec::new()).err(),
            Some(PlanError::StateMismatch)
        );

        // Or(A: Mixed(Window, Window), B: Window) with A as a sailor.
        let layout = [
            NodeKind::Or,
            NodeKind::Mixed { children: 2 },
            NodeKind::Window,
            NodeKind::Window,
            NodeKind::Window,
        ];
        let entities = entities(1);
        let row = vec![
            NodeState::Or {
                branch: OrBranch::B,
            },
            NodeState::Sailor,
            window(0),
            NodeState::Sailor,
            window(0),
        ];
        let result = Plan::new(&layout, vec![segment(&entities, 0)], row, Vec::new());

        assert_eq!(result.err(), Some(PlanError::StateMismatch));
    }

    /// Asserts that a nested `Mixed` keeps its own captain mask:
    /// the outer mask flags its immediate children, the inner one flags its own.
    #[test]
    fn nested_masks() {
        // Mixed(Window, Mixed(Window, Window)), the inner Mixed drives with its second child.
        let layout = [
            NodeKind::Mixed { children: 2 },
            NodeKind::Window,
            NodeKind::Mixed { children: 2 },
            NodeKind::Window,
            NodeKind::Window,
        ];
        let entities = entities(1);
        let row = vec![
            mixed(0b10, 0, 0),
            NodeState::Sailor,
            mixed(0b10, 0, 0),
            NodeState::Sailor,
            window(0),
        ];

        assert!(Plan::new(&layout, vec![segment(&entities, 0)], row, Vec::new()).is_ok());
    }

    /// Asserts that empty captain masks and masks flagging missing children are rejected.
    #[test]
    fn captain_mask() {
        for captain_mask in [0, 0b100, 1 << 31] {
            let rows = vec![mixed(captain_mask, 0, 0), window(0), window(0)];

            assert_eq!(
                mixed_plan(rows, Vec::new()).err(),
                Some(PlanError::InvalidCaptainMask)
            );
        }
    }

    /// Asserts that a `Mixed` with 32 children accepts its last child as captain.
    #[test]
    fn mask_width() {
        let mut layout = vec![NodeKind::Mixed { children: 32 }];
        layout.extend([NodeKind::Window; 32]);
        let mut row = vec![mixed(1 << 31, 0, 0)];
        row.extend([NodeState::Sailor; 31]);
        row.push(window(0));
        let entities = entities(1);

        assert!(Plan::new(&layout, vec![segment(&entities, 0)], row, Vec::new()).is_ok());
    }

    /// Asserts that layouts that aren't a single preorder tree are rejected:
    /// an empty layout, missing children, extra nodes, and `Mixed`s with 0 or 33 children.
    #[test]
    fn layout() {
        let entities = entities(1);
        let or_a = NodeState::Or {
            branch: OrBranch::A,
        };
        let cases: [(&[NodeKind], Vec<NodeState>); 5] = [
            (&[], Vec::new()),
            (&[NodeKind::Or, NodeKind::Window], vec![or_a, window(0)]),
            (
                &[NodeKind::Window, NodeKind::Window],
                vec![window(0), NodeState::Sailor],
            ),
            (&[NodeKind::Mixed { children: 0 }], vec![mixed(0b1, 0, 0)]),
            (&[NodeKind::Mixed { children: 33 }], vec![mixed(0b1, 0, 0)]),
        ];

        for (layout, row) in cases {
            let result = Plan::new(layout, vec![segment(&entities, 0)], row, Vec::new());

            assert_eq!(result.err(), Some(PlanError::MalformedLayout));
        }
    }
}
