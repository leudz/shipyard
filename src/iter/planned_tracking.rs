use super::{EntityCursor, SegmentEntry, SegmentRow, Shiperator};
use crate::{EntityId, sparse_set::tracking_plan::TrackingPlan};
use core::ops::Range;

pub(crate) trait TrackingFilter: Shiperator {
    fn segment_start(&self) -> usize;
}

/// A tracking filter with a query-local candidate plan.
#[doc(hidden)]
#[derive(Clone)]
pub struct PlannedTracking<S> {
    pub(crate) inner: S,
    pub(crate) plan: TrackingPlan,
    pub(crate) len: usize,
}

impl<S: TrackingFilter> Shiperator for PlannedTracking<S> {
    type Out = S::Out;
    type Index = S::Index;
    const PREFER_CAPTAIN_FIRST: bool = S::PREFER_CAPTAIN_FIRST;
    const HAS_CANDIDATE_PLAN: bool = true;

    #[inline]
    fn node_count() -> usize {
        S::node_count()
    }
    #[inline]
    fn describe(&self, active: bool, row: &mut SegmentRow) {
        self.inner.describe(active, row);
    }
    #[inline]
    unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
        // SAFE: The wrapper preserves its filter's layout and segment state.
        unsafe { self.inner.enter_segment(entry) };
    }
    #[inline]
    fn sail_time(&self) -> usize {
        self.plan.scan_cost()
    }
    #[inline]
    fn is_exact_sized(&self) -> bool {
        false
    }
    #[inline]
    fn has_no_candidates(&self) -> bool {
        self.plan.is_empty()
    }
    #[inline]
    fn has_dense_candidates(&self) -> bool {
        matches!(self.plan, TrackingPlan::Dense { .. })
    }
    #[inline]
    fn unpick(&mut self) {
        if !self.plan.is_empty() {
            self.plan = TrackingPlan::Dense { len: self.len };
        }
    }
    #[inline]
    unsafe fn next_candidate_range(&self, start: usize, end: usize) -> Range<usize> {
        let offset = self.inner.segment_start();
        let range = self.plan.next_range(start + offset, end + offset);
        range.start - offset..range.end - offset
    }
    #[inline]
    unsafe fn candidate_count(&self, start: usize, end: usize) -> usize {
        let offset = self.inner.segment_start();
        self.plan.count(start + offset, end + offset)
    }
    #[inline]
    unsafe fn candidate_split_at(&self, start: usize, end: usize, count: usize) -> usize {
        let offset = self.inner.segment_start();
        self.plan.split_at(start + offset, end + offset, count) - offset
    }
    #[inline]
    unsafe fn captain_indices_of(
        &self,
        entities: &EntityCursor,
        index: usize,
    ) -> Option<Self::Index> {
        // SAFE: Our caller's cursor and index are the filter's own.
        unsafe { self.inner.captain_indices_of(entities, index) }
    }
    #[inline]
    fn sailor_indices_of(&self, entity: EntityId) -> Option<Self::Index> {
        if self.plan.is_empty() {
            None
        } else {
            self.inner.sailor_indices_of(entity)
        }
    }
    #[inline]
    fn storage_index(index: &Self::Index) -> Option<usize> {
        S::storage_index(index)
    }
    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        // SAFE: The index was resolved by this same filter.
        unsafe { self.inner.get_data(index) }
    }
}
