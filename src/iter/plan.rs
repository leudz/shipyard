//! Shared immutable segment plans.
//!
//! A [`Plan`] describes a traversal split into segments: the entities each segment walks,
//! the state every typed node has to install when entering it and the regions earlier
//! segments already own.\
//! Once published, a plan never changes. [`SegmentSchedule`]s share it and each tracks
//! its own position in the follow-up segments, independently of the active `EntityCursor`.
#[cfg(test)]
mod tests;

use crate::entity_id::EntityId;
use crate::iter::Shiperator;
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
pub(crate) enum OrBranch {
    A,
    B,
}

/// State a node installs when entering a segment.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

impl NodeState {
    /// Moves a `Mixed`'s coverage span `offset` records further in the pool.
    ///
    /// Used when a child plan's coverage pool is appended to a larger one.
    fn with_coverage_offset(self, offset: usize) -> NodeState {
        match self {
            NodeState::Mixed {
                captain_mask,
                coverage,
            } => NodeState::Mixed {
                captain_mask,
                coverage: CoverageSpan {
                    start: coverage.start + offset,
                    len: coverage.len,
                },
            },
            state => state,
        }
    }
}

/// Range of records in a plan's coverage pool.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct CoverageSpan {
    pub(crate) start: usize,
    pub(crate) len: usize,
}

impl CoverageSpan {
    /// Span without any record.
    pub(crate) const EMPTY: CoverageSpan = CoverageSpan { start: 0, len: 0 };
}

/// Physical region of a storage owned by an earlier segment.
///
/// The earlier segment already walked every candidate whose component in that storage
/// lies in the region, later segments must reject them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Coverage {
    /// Immediate child of the `Mixed` holding this record, whose storage contains the region.
    ///
    /// It's always a window, other nodes don't have a single storage.
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

    /// Returns a descriptor over the entities of `cursor`, using row `row`.
    ///
    /// Segments restart local indices at 0, so `cursor` must not have consumed any entity yet.
    #[inline]
    pub(crate) fn from_cursor(cursor: EntityCursor, row: usize) -> Self {
        assert_eq!(cursor.local_index(), 0, "segments start at local index 0");

        // SAFE: `cursor`'s creator already guarantees its `len` entities are initialized and stay
        // valid and unmodified while it or a copy of it is used. The descriptor only hands out
        // cursors over these same entities.
        unsafe { SegmentDescriptor::new(cursor.as_ptr(), cursor.len(), row) }
    }

    /// Returns a cursor over the segment's entities.
    #[inline]
    pub(crate) fn cursor(&self) -> EntityCursor {
        // SAFE: `new`'s caller guarantees the entities are valid and stay unmodified
        // while this descriptor and the cursors obtained from it are used.
        unsafe { EntityCursor::new(self.entities, self.len) }
    }

    /// Returns the descriptor of the segment's first `len` candidates.
    ///
    /// It keeps the segment's row and its local indices: candidate `i` of the prefix is
    /// candidate `i` of the segment.
    ///
    /// ### Panics
    ///
    /// - `len` is 0 or larger than the segment's length.
    #[inline]
    fn prefix(self, len: usize) -> SegmentDescriptor {
        assert!(
            0 < len && len <= self.len,
            "a prefix keeps at least one of the segment's candidates"
        );

        SegmentDescriptor { len, ..self }
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
    /// A coverage record has an empty region or names a child its `Mixed` doesn't have
    /// or that isn't a window.
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

// SAFE: Plans only point to entities, never to components.
// `SegmentDescriptor::new`'s caller guarantees these entities aren't modified while the plan is used
// and the plan only reads them. `EntityId` is plain data, reading it from any thread is fine.
// Mutable component access stays in the typed windows and their own `Send` bounds,
// sharing a plan doesn't give another way to reach overlapping mutable outputs.
unsafe impl Send for Plan {}
unsafe impl Sync for Plan {}

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

    /// Returns every segment, the ones schedules already took included.
    #[cfg(test)]
    pub(crate) fn segments(&self) -> &[SegmentDescriptor] {
        &self.segments
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

    // Bit `i` flags child `i` as a window.
    let mut window_children = 0u32;
    let mut next = node + 1;
    for child in 0..arity {
        if layout.get(next) == Some(&NodeKind::Window) {
            window_children |= 1 << child;
        }

        let child_active = active_children & (1 << child) != 0;
        next = validate_subtree(layout, row, coverage, next, child_active)?;
    }

    // Coverage is matched against a child's storage index, only windows have one.
    if let (true, NodeState::Mixed { coverage: span, .. }) = (active, state) {
        let records = coverage_records(coverage, span).ok_or(PlanError::CoverageOutOfBounds)?;

        if records
            .iter()
            .any(|record| window_children & (1 << record.child) == 0)
        {
            return Err(PlanError::InvalidCoverage);
        }
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

/// Shared ownership of a [`Plan`] and of an interval of its segments, the follow-ups.
///
/// Each schedule advances independently, the plan itself is never modified.\
/// The interval can end inside a segment, the schedule then only owns that segment's first
/// `end_len` candidates, see [`split_follow_ups_at`](SegmentSchedule::split_follow_ups_at).
/// It never starts inside one: whoever owns a segment's suffix walks it with its own cursor.\
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
    /// Index one past the last segment we own.
    end: usize,
    /// Number of candidates we own in segment `end - 1`, all of them unless a split cut it.
    ///
    /// Only read while `next < end`.
    end_len: usize,
    /// Number of candidates in the follow-up segments.
    remaining_len: usize,
}

impl SegmentSchedule {
    /// Publishes `plan`, all its segments are follow-ups.
    #[inline]
    pub(crate) fn new(plan: Plan) -> SegmentSchedule {
        let remaining_len = plan.total_len;
        let end = plan.segments.len();
        // Plans have at least one segment.
        let end_len = plan.segments[end - 1].len;

        SegmentSchedule {
            plan: ManuallyDrop::new(Arc::new(plan)),
            next: 0,
            end,
            end_len,
            remaining_len,
        }
    }

    /// Returns the shared plan.
    #[inline]
    pub(crate) fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Returns the follow-up segments left, in order.
    ///
    /// They're the parts we own: the last one can be a prefix of its segment.
    #[inline]
    pub(crate) fn follow_ups(&self) -> impl ExactSizeIterator<Item = SegmentDescriptor> + '_ {
        (self.next..self.end).map(|index| self.owned(index))
    }

    /// Returns `true` when a follow-up segment is left.
    #[inline]
    pub(crate) fn has_follow_ups(&self) -> bool {
        self.next < self.end
    }

    /// Returns the part we own of segment `index`, one of `next..end`.
    #[inline]
    fn owned(&self, index: usize) -> SegmentDescriptor {
        debug_assert!(self.next <= index && index < self.end);

        let segment = self.plan.segments[index];
        if index + 1 == self.end {
            segment.prefix(self.end_len)
        } else {
            segment
        }
    }

    /// Consumes and returns the next follow-up segment.
    #[inline]
    pub(crate) fn take_next(&mut self) -> Option<SegmentDescriptor> {
        if !self.has_follow_ups() {
            return None;
        }
        let segment = self.owned(self.next);

        self.next += 1;
        // `remaining_len` is the sum of the lengths of the segments from the old `self.next`,
        // `segment` is one of them so this can't underflow.
        self.remaining_len -= segment.len;

        Some(segment)
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
    #[cfg_attr(not(feature = "parallel"), allow(dead_code))]
    pub(crate) fn split_follow_ups(&mut self) -> SegmentSchedule {
        let follow_ups = self.clone();

        self.next = self.end;
        self.remaining_len = 0;

        follow_ups
    }

    /// Splits the follow-ups after their first `count` candidates.
    ///
    /// `self` keeps these `count` candidates, ending with a prefix of the segment the cut falls in
    /// when it isn't at a segment boundary. The rest starts with the segment holding the cut:
    /// the returned [`FollowUpCut`] has that segment, the local index of its first candidate
    /// after the cut and a schedule of every segment after it.\
    /// Nothing is allocated, both schedules share the plan.
    ///
    /// ### Panics
    ///
    /// - `count` isn't smaller than the number of candidates in the follow-ups.
    #[cfg_attr(not(feature = "parallel"), allow(dead_code))]
    pub(crate) fn split_follow_ups_at(&mut self, count: usize) -> FollowUpCut {
        assert!(
            count < self.remaining_len,
            "the rest of the follow-ups keeps at least one candidate"
        );

        // Finds the segment holding candidate `count`, counting the candidates before it.
        let mut index = self.next;
        let mut before = 0;
        let segment = loop {
            let segment = self.owned(index);
            if count < before + segment.len {
                break segment;
            }

            before += segment.len;
            index += 1;
        };
        let local_index = count - before;

        let rest = SegmentSchedule {
            plan: ManuallyDrop::new(Arc::clone(&self.plan)),
            next: index + 1,
            end: self.end,
            end_len: self.end_len,
            remaining_len: self.remaining_len - before - segment.len,
        };

        // We keep the segments before the cut, and the prefix of `segment` before it if there's one.
        // A cut at a boundary leaves us ending on the previous segment, which isn't our old last
        // one since the cut leaves at least one candidate after it, so we own all of it.
        if local_index == 0 {
            self.end = index;
            self.end_len = if index > self.next {
                self.plan.segments[index - 1].len
            } else {
                0
            };
        } else {
            self.end = index + 1;
            self.end_len = local_index;
        }
        self.remaining_len = count;

        FollowUpCut {
            segment,
            local_index,
            rest,
        }
    }

    /// Returns a weak reference to the shared plan, to check when it's released.
    #[cfg(test)]
    pub(crate) fn downgrade(&self) -> alloc::sync::Weak<Plan> {
        Arc::downgrade(&self.plan)
    }
}

impl Clone for SegmentSchedule {
    #[inline]
    fn clone(&self) -> Self {
        SegmentSchedule {
            plan: ManuallyDrop::new(Arc::clone(&self.plan)),
            next: self.next,
            end: self.end,
            end_len: self.end_len,
            remaining_len: self.remaining_len,
        }
    }
}

/// Where [`SegmentSchedule::split_follow_ups_at`] cut the follow-ups, and what follows the cut.
#[cfg_attr(not(feature = "parallel"), allow(dead_code))]
pub(crate) struct FollowUpCut {
    /// The segment holding the cut, with its original row and local indices.
    ///
    /// It's the part the schedule owned, it can be a prefix of the plan's segment.
    pub(crate) segment: SegmentDescriptor,
    /// Local index in `segment` of the first candidate after the cut.
    pub(crate) local_index: usize,
    /// The follow-ups after `segment`.
    pub(crate) rest: SegmentSchedule,
}

impl Drop for SegmentSchedule {
    #[inline(always)]
    fn drop(&mut self) {
        // SAFE: `plan` is initialized by every constructor and only taken here.
        // `drop` runs once and nothing reads `self` afterwards.
        let plan = unsafe { ManuallyDrop::take(&mut self.plan) };

        drop(plan);
    }
}

/// Node states read in preorder while a query enters a segment.
///
/// Each node takes its own state then hands the reader to its active children.
/// Sailor subtrees are skipped as a whole.
#[doc(hidden)]
pub struct SegmentEntry<'a> {
    plan: &'a Plan,
    states: &'a [NodeState],
}

impl<'a> SegmentEntry<'a> {
    /// Returns a reader over the row `segment` installs.
    ///
    /// `segment` must come from `plan`.
    #[inline]
    pub(crate) fn new(plan: &'a Plan, segment: &SegmentDescriptor) -> SegmentEntry<'a> {
        SegmentEntry {
            plan,
            states: plan.row(segment),
        }
    }

    /// Returns the state of the next node in preorder.
    #[inline]
    fn next_state(&mut self) -> NodeState {
        let (&state, rest) = self
            .states
            .split_first()
            .expect("plan rows have one state per node");
        self.states = rest;

        state
    }

    /// Returns the segment start of the next node, an active window.
    #[inline]
    pub(crate) fn next_window(&mut self) -> usize {
        match self.next_state() {
            NodeState::Window { segment_start } => segment_start,
            state => unreachable!("expected an active window state, got {state:?}"),
        }
    }

    /// Returns the captain mask and coverage records of the next node, an active `Mixed`.
    ///
    /// The records live in the plan, they don't move while it's alive.
    #[inline]
    pub(crate) fn next_mixed(&mut self) -> (u32, &'a [Coverage]) {
        match self.next_state() {
            NodeState::Mixed {
                captain_mask,
                coverage,
            } => (captain_mask, self.plan.coverage(coverage)),
            state => unreachable!("expected an active Mixed state, got {state:?}"),
        }
    }

    /// Returns the branch of the next node, an active `Or`.
    #[inline]
    pub(crate) fn next_or(&mut self) -> OrBranch {
        match self.next_state() {
            NodeState::Or { branch } => branch,
            state => unreachable!("expected an active Or state, got {state:?}"),
        }
    }

    /// Skips the `node_count` nodes of a sailor subtree.
    #[inline]
    pub(crate) fn skip(&mut self, node_count: usize) {
        self.states = &self.states[node_count..];
    }

    /// Returns `true` once every node took its state.
    #[inline]
    pub(crate) fn is_done(&self) -> bool {
        self.states.is_empty()
    }
}

/// Kinds and current states of a query's nodes, in preorder.
///
/// Composite nodes use it to describe themselves when they embed their children's segments.
#[doc(hidden)]
pub struct SegmentRow {
    layout: Vec<NodeKind>,
    states: Vec<NodeState>,
}

impl SegmentRow {
    /// Returns the row describing `shiperator` as the root of a query.
    pub(crate) fn of<S: Shiperator>(shiperator: &S) -> SegmentRow {
        let mut row = SegmentRow {
            layout: Vec::new(),
            states: Vec::new(),
        };
        shiperator.describe(true, &mut row);

        row
    }

    /// Appends a node.
    #[inline]
    pub(crate) fn push(&mut self, kind: NodeKind, state: NodeState) {
        self.layout.push(kind);
        self.states.push(state);
    }

    /// Appends a window node, `segment_start` is only used when it's `active`.
    #[inline]
    pub(crate) fn push_window(&mut self, active: bool, segment_start: usize) {
        let state = if active {
            NodeState::Window { segment_start }
        } else {
            NodeState::Sailor
        };

        self.push(NodeKind::Window, state);
    }

    pub(crate) fn layout(&self) -> &[NodeKind] {
        &self.layout
    }

    pub(crate) fn states(&self) -> &[NodeState] {
        &self.states
    }
}

/// Collects the segments of a composite query before publishing them as a single plan.
///
/// Composite nodes (`Or`, `Mixed`) use it when they're built, to embed the segments of their
/// children under their own state.
pub(crate) struct PlanBuilder {
    layout: Vec<NodeKind>,
    segments: Vec<SegmentDescriptor>,
    rows: Vec<NodeState>,
    coverage: Vec<Coverage>,
}

impl PlanBuilder {
    /// Returns a builder for a query with `layout`.
    pub(crate) fn new(layout: &[NodeKind]) -> PlanBuilder {
        PlanBuilder {
            layout: layout.to_vec(),
            segments: Vec::new(),
            rows: Vec::new(),
            coverage: Vec::new(),
        }
    }

    /// Adds a segment walking the entities of `cursor` and installing `row`.
    ///
    /// `cursor` must not have consumed any entity yet.
    pub(crate) fn push_segment(&mut self, cursor: EntityCursor, row: &[NodeState]) {
        debug_assert_eq!(row.len(), self.layout.len());

        let row_index = self.rows.len() / self.layout.len();
        self.rows.extend_from_slice(row);
        self.segments
            .push(SegmentDescriptor::from_cursor(cursor, row_index));
    }

    /// Adds the follow-ups left in a child's `schedule`.
    ///
    /// `parent_row` is the row of the whole query while the child drives it,
    /// the child's states start at index `slot`.\
    /// Each follow-up installs `parent_row` with the child's part replaced by the follow-up's own row.
    pub(crate) fn push_child_follow_ups(
        &mut self,
        schedule: &SegmentSchedule,
        parent_row: &[NodeState],
        slot: usize,
    ) {
        let plan = schedule.plan();
        let after_child = slot + plan.row_width;
        debug_assert_eq!(parent_row.len(), self.layout.len());
        debug_assert!(after_child <= parent_row.len());

        // Coverage records are relative to the `Mixed` holding them, they keep their meaning
        // under a new parent. Only the spans pointing into the pool have to move.
        let coverage_offset = self.coverage.len();
        self.coverage.extend_from_slice(&plan.coverage);

        // We copy the parts the child owns, a bounded schedule's last one can be a prefix.
        for segment in schedule.follow_ups() {
            let row_index = self.rows.len() / self.layout.len();

            self.rows.extend_from_slice(&parent_row[..slot]);
            self.rows.extend(
                plan.row(&segment)
                    .iter()
                    .map(|state| state.with_coverage_offset(coverage_offset)),
            );
            self.rows.extend_from_slice(&parent_row[after_child..]);

            self.segments.push(SegmentDescriptor {
                row: row_index,
                ..segment
            });
        }
    }

    /// Publishes the plan, `None` when no segment was added.
    pub(crate) fn finish(self) -> Option<SegmentSchedule> {
        if self.segments.is_empty() {
            return None;
        }

        let plan = Plan::new(&self.layout, self.segments, self.rows, self.coverage)
            .expect("composite plans embed valid rows in their parent's layout");

        Some(SegmentSchedule::new(plan))
    }
}
