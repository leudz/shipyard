use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::Shiperator;
use crate::iter::{EntityCursor, SegmentEntry, SegmentRow};
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::tracking::{
    Inserted, InsertedOrModified, InsertionOrModificationTracking, InsertionTracking,
    ModificationTracking, Modified,
};

const TRACKING_FACTOR: f32 = 2.0;

/// Implements `Shiperator` for tracking filters over both kinds of windows.
///
/// Each filter only exists for windows whose `Track` has its `$capability`, so the storage
/// keeps the timestamps `$is_tracked` reads.
macro_rules! impl_tracking_filter {
    ($($filter: ident: $capability: ident, $is_tracked: ident;)+) => {$(
        impl_tracking_filter!(@window $filter, $capability, $is_tracked, FullRawWindow);
        impl_tracking_filter!(@window $filter, $capability, $is_tracked, FullRawWindowMut);
    )+};
    (@window $filter: ident, $capability: ident, $is_tracked: ident, $window: ident) => {
        impl<'tmp, T: Component, Track: $capability> Shiperator for $filter<$window<'tmp, T, Track>>
        where
            $window<'tmp, T, Track>: Shiperator<Index = usize>,
        {
            type Out = <$window<'tmp, T, Track> as Shiperator>::Out;
            type Index = usize;

            #[inline]
            fn node_count() -> usize {
                1
            }

            #[inline]
            fn describe(&self, active: bool, row: &mut SegmentRow) {
                self.0.describe_window(active, row);
            }

            #[inline]
            unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
                // SAFE: Our caller guarantees `entry` holds our window's state for the segment about to be iterated.
                unsafe { self.0.enter_window(entry) };
            }

            #[inline]
            #[allow(clippy::cast_precision_loss)]
            fn sail_time(&self) -> usize {
                (self.0.sail_time() as f32 * TRACKING_FACTOR) as usize
            }

            #[inline]
            fn is_exact_sized(&self) -> bool {
                false
            }

            #[inline]
            unsafe fn captain_indices_of(
                &self,
                _entities: &EntityCursor,
                index: usize,
            ) -> Option<Self::Index> {
                // Timestamps are stored at the component's absolute index.
                let index = self.0.resolve_index(index);

                // SAFE: Our caller passes a local index of the segment we entered, whose start
                // `enter_segment`'s caller picked so it resolves to one of our storage's components.
                unsafe { self.0.$is_tracked(index) }.then_some(index)
            }

            #[inline]
            fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
                let index = self.0.index_of(eid)?;

                // SAFE: `index_of` only returns indices of our storage's components.
                unsafe { self.0.$is_tracked(index) }.then_some(index)
            }

            #[inline]
            fn storage_index(_index: &Self::Index) -> Option<usize> {
                None
            }

            #[inline]
            unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
                // SAFE: Our caller passes an index we resolved, our window resolves it the same way.
                unsafe { self.0.get_data(index) }
            }
        }
    };
}

impl_tracking_filter![
    Inserted: InsertionTracking, is_inserted;
    Modified: ModificationTracking, is_modified;
    InsertedOrModified: InsertionOrModificationTracking, is_inserted_or_modified;
];
