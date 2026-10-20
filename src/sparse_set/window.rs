#[cfg(test)]
mod tests;

use crate::atomic_refcell::{ExclusiveBorrow, SharedBorrow};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::{ConstructionMetadata, RawEntityIdAccess, SegmentEntry, SegmentRow};
use crate::sparse_set::sparse_array::Page;
use crate::tracking::{Tracking, TrackingTimestamp};
use crate::views::{View, ViewMut};
use alloc::boxed::Box;
use core::marker::PhantomData;
use core::ptr::{self, NonNull};

/// Raw access to a storage's components.
///
/// All pointers are anchored at the start of the storage's allocations.\
/// Captains iterate a segment of the storage: they receive indices local to that segment
/// and resolve them to absolute storage indices by adding `segment_start`.\
/// Sparse lookups always return absolute indices.
///
/// `Track` is the tracking of the view the window comes from. Borrowing that view checked
/// the storage tracks at least what `Track` does, so its timestamps exist for every component.
pub struct FullRawWindow<'a, T, Track: Tracking> {
    sparse: *const *const EntityId,
    sparse_len: usize,
    pub(crate) dense: NonNull<EntityId>,
    pub(crate) dense_len: usize,
    /// Absolute index of the first component of the segment this window is iterating.
    pub(crate) segment_start: usize,
    pub(crate) data: *const T,
    pub(crate) insertion_data: *const TrackingTimestamp,
    pub(crate) modification_data: *const TrackingTimestamp,
    pub(crate) last_insertion: TrackingTimestamp,
    pub(crate) last_modification: TrackingTimestamp,
    pub(crate) current: TrackingTimestamp,
    _phantom: PhantomData<(&'a T, Track)>,
}

// SAFE: A window only gives shared access to its storage's components, `&T` is `Send` when `T`
// is `Sync`. The other thread can only read the storage's entities, sparse pages and timestamps,
// which stay borrowed and unmodified for 'a.
unsafe impl<T: Sync + Component, Track: Tracking> Send for FullRawWindow<'_, T, Track> {}

/// Asserts that an iterator over `!Sync` components isn't `Send`, the view could still reach the
/// same components from this thread.
///
/// ```compile_fail
/// use shipyard::{borrow::NonSync, track, Component, IntoIter, View, World};
/// use std::cell::Cell;
///
/// struct Counter(Cell<u32>);
/// impl Component for Counter {
///     type Tracking = track::Untracked;
/// }
///
/// fn assert_send<T: Send>(_: &T) {}
///
/// let world = World::new();
/// world.run(|counters: NonSync<View<Counter>>| {
///     assert_send(&(&*counters).iter());
/// });
/// ```
#[cfg(all(doctest, feature = "thread_local"))]
struct NonSyncIterNotSend;

impl<'w, T: Component, Track: Tracking> FullRawWindow<'w, T, Track> {
    #[inline]
    pub(crate) fn from_view(view: &View<'_, T, Track>) -> Self {
        let sparse_len = view.sparse.len();
        let sparse: *const Option<Box<Page>> = view.sparse.as_ptr();
        let sparse = sparse as *const *const EntityId;

        FullRawWindow {
            sparse,
            sparse_len,
            dense: NonNull::new(view.dense.as_ptr().cast_mut()).unwrap(),
            dense_len: view.dense.len(),
            segment_start: 0,
            data: view.data.as_ptr(),
            insertion_data: view.insertion_data.as_ptr(),
            modification_data: view.modification_data.as_ptr(),
            last_insertion: view.last_insertion,
            last_modification: view.last_modification,
            current: view.current,
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub(crate) fn from_owned_view(
        view: View<'_, T, Track>,
    ) -> (Self, Option<SharedBorrow<'_>>, SharedBorrow<'_>) {
        let View {
            sparse_set,
            all_borrow,
            borrow,
            last_insertion,
            last_modification,
            current,
            ..
        } = view;

        let sparse_len = sparse_set.sparse.len();
        let sparse: *const Option<Box<Page>> = sparse_set.sparse.as_ptr();
        let sparse = sparse as *const *const EntityId;

        (
            FullRawWindow {
                sparse,
                sparse_len,
                dense: NonNull::new(sparse_set.dense.as_ptr().cast_mut()).unwrap(),
                dense_len: sparse_set.dense.len(),
                segment_start: 0,
                data: sparse_set.data.as_ptr(),
                insertion_data: sparse_set.insertion_data.as_ptr(),
                modification_data: sparse_set.modification_data.as_ptr(),
                last_insertion,
                last_modification,
                current,
                _phantom: PhantomData,
            },
            all_borrow,
            borrow,
        )
    }

    #[inline]
    pub(crate) fn from_view_mut(view: &ViewMut<'_, T, Track>) -> Self {
        let sparse_len = view.sparse.len();
        let sparse: *const Option<Box<Page>> = view.sparse.as_ptr();
        let sparse = sparse as *const *const EntityId;

        FullRawWindow {
            sparse,
            sparse_len,
            dense: NonNull::new(view.dense.as_ptr().cast_mut()).unwrap(),
            dense_len: view.dense.len(),
            segment_start: 0,
            data: view.data.as_ptr(),
            insertion_data: view.insertion_data.as_ptr(),
            modification_data: view.modification_data.as_ptr(),
            last_insertion: view.last_insertion,
            last_modification: view.last_modification,
            current: view.current,
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub(crate) fn index_of(&self, entity: EntityId) -> Option<usize> {
        if let Some(sparse_entity) = self.sparse_index(entity) {
            if entity.generation() == sparse_entity.generation() {
                Some(sparse_entity.uindex())
            } else {
                None
            }
        } else {
            None
        }
    }

    #[inline]
    fn sparse_index(&self, entity: EntityId) -> Option<EntityId> {
        if entity.bucket() < self.sparse_len {
            // SAFE: `sparse` points to the `sparse_len` pages of the storage we borrow and
            // `bucket` is below that length. A page is an `Option<Box<Page>>`,
            // which is a nullable pointer to the page.
            let bucket = unsafe { ptr::read(self.sparse.add(entity.bucket())) };

            if !bucket.is_null() {
                // SAFE: `bucket` points to an allocated page, `Page` is `repr(C)` around its
                // `BUCKET_SIZE` entities and `bucket_index` is a remainder of `BUCKET_SIZE`.
                Some(unsafe { ptr::read(bucket.add(entity.bucket_index())) })
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns an entity source over the whole storage.
    #[inline]
    pub(crate) fn entity_iter(&self) -> RawEntityIdAccess {
        // SAFE: `dense` points to the `dense_len` entities of the storage this window borrows for `'w`.
        // The access is only used by iterators holding that borrow, which prevents any modification.
        unsafe { RawEntityIdAccess::new(self.dense, self.dense_len) }
    }

    /// Converts an index local to the current segment into an absolute storage index.
    #[inline]
    pub(crate) fn resolve_index(&self, local_index: usize) -> usize {
        self.segment_start + local_index
    }

    /// Makes the window iterate the segment starting at absolute index `segment_start`.
    ///
    /// ### Safety
    ///
    /// For every local index `i` this window will receive as a captain until the next call,
    /// `segment_start + i` must be smaller than the storage's length.\
    /// The entity source driving the iteration must hold, at local index `i`,
    /// the entity stored at absolute index `segment_start + i` in this window.
    #[inline]
    pub(crate) unsafe fn set_segment_start(&mut self, segment_start: usize) {
        self.segment_start = segment_start;
    }

    /// Appends this window's node to `row`.
    #[inline]
    pub(crate) fn describe_window(&self, active: bool, row: &mut SegmentRow) {
        row.push_window(active, self.segment_start);
    }

    /// Installs the segment start `entry` holds for this window.
    ///
    /// ### Safety
    ///
    /// `entry`'s next state must be this window's state for the segment about to be iterated.
    /// It must uphold [`set_segment_start`](Self::set_segment_start)'s requirements.
    #[inline]
    pub(crate) unsafe fn enter_window(&mut self, entry: &mut SegmentEntry<'_>) {
        let segment_start = entry.next_window();

        // SAFE: Our caller guarantees this is our segment start for the segment about to be iterated,
        // meeting `set_segment_start`'s requirements.
        unsafe { self.set_segment_start(segment_start) };
    }

    /// Returns `true` when the component at `index` was inserted in this window's tracking interval.
    ///
    /// Always `false` when `Track` doesn't track insertion.
    ///
    /// ### Safety
    ///
    /// `index` must be smaller than the storage's length.
    #[inline]
    pub(crate) unsafe fn is_inserted(&self, index: usize) -> bool {
        Track::track_insertion() && {
            // SAFE: `Track` tracks insertion and the storage was checked to track at least what
            // `Track` does, so it keeps an insertion timestamp per component.
            // Our caller guarantees `index` is one of them.
            unsafe { *self.insertion_data.add(index) }.is_within(self.last_insertion, self.current)
        }
    }

    /// Returns `true` when the component at `index` was modified in this window's tracking interval.
    ///
    /// Always `false` when `Track` doesn't track modification.
    ///
    /// ### Safety
    ///
    /// `index` must be smaller than the storage's length.
    #[inline]
    pub(crate) unsafe fn is_modified(&self, index: usize) -> bool {
        Track::track_modification() && {
            // SAFE: Same as `is_inserted`, with the modification timestamps.
            unsafe { *self.modification_data.add(index) }
                .is_within(self.last_modification, self.current)
        }
    }

    /// Returns `true` when the component at `index` was inserted or modified in this window's
    /// tracking interval, only checking the kinds `Track` tracks.
    ///
    /// ### Safety
    ///
    /// `index` must be smaller than the storage's length.
    #[inline]
    pub(crate) unsafe fn is_inserted_or_modified(&self, index: usize) -> bool {
        // SAFE: Our caller's guarantee is the one both checks need.
        unsafe { self.is_inserted(index) || self.is_modified(index) }
    }
}

impl<T: Component, Track: Tracking> Clone for FullRawWindow<'_, T, Track> {
    #[inline]
    fn clone(&self) -> Self {
        FullRawWindow {
            sparse: self.sparse,
            sparse_len: self.sparse_len,
            dense: self.dense,
            dense_len: self.dense_len,
            segment_start: self.segment_start,
            data: self.data,
            insertion_data: self.insertion_data,
            modification_data: self.modification_data,
            last_insertion: self.last_insertion,
            last_modification: self.last_modification,
            current: self.current,
            _phantom: PhantomData,
        }
    }
}

/// Raw exclusive access to a storage's components.
///
/// Uses the same coordinates as [`FullRawWindow`].
pub struct FullRawWindowMut<'a, T, Track: Tracking> {
    sparse: *mut *mut EntityId,
    sparse_len: usize,
    pub(crate) dense: NonNull<EntityId>,
    pub(crate) dense_len: usize,
    /// Absolute index of the first component of the segment this window is iterating.
    pub(crate) segment_start: usize,
    pub(crate) data: *mut T,
    pub(crate) insertion_data: *const TrackingTimestamp,
    pub(crate) modification_data: *mut TrackingTimestamp,
    pub(crate) last_insertion: TrackingTimestamp,
    pub(crate) last_modification: TrackingTimestamp,
    pub(crate) current: TrackingTimestamp,
    pub(crate) is_tracking_modification: bool,
    _phantom: PhantomData<(&'a mut T, Track)>,
}

// SAFE: A window gives exclusive access to the components of the storage it borrows mutably for
// 'a, sending it moves that access to the other thread. `&mut T` is `Send` when `T` is `Send`.
unsafe impl<T: Send + Component, Track: Tracking> Send for FullRawWindowMut<'_, T, Track> {}

impl<'w, T: Component, Track: Tracking> FullRawWindowMut<'w, T, Track> {
    #[inline]
    pub(crate) fn new(view: &mut ViewMut<'_, T, Track>) -> Self {
        let sparse_len = view.sparse.len();
        let sparse: *mut Option<Box<Page>> = view.sparse.as_mut_ptr();
        let sparse = sparse as *mut *mut EntityId;

        FullRawWindowMut {
            sparse,
            sparse_len,
            dense: NonNull::new(view.dense.as_mut_ptr().cast()).unwrap(),
            dense_len: view.dense.len(),
            segment_start: 0,
            data: view.data.as_mut_ptr(),
            insertion_data: view.insertion_data.as_ptr(),
            modification_data: view.modification_data.as_mut_ptr(),
            last_insertion: view.last_insertion,
            last_modification: view.last_modification,
            current: view.current,
            is_tracking_modification: view.is_tracking_modification(),
            _phantom: PhantomData,
        }
    }

    /// Returns the window over `view`'s storage, the guards keeping it borrowed and the storage's
    /// construction metadata.
    #[inline]
    pub(crate) fn new_owned(
        view: ViewMut<'_, T, Track>,
    ) -> (
        Self,
        Option<SharedBorrow<'_>>,
        ExclusiveBorrow<'_>,
        ConstructionMetadata,
    ) {
        let ViewMut {
            sparse_set,
            all_borrow,
            borrow,
            last_insertion,
            last_modification,
            current,
            ..
        } = view;

        let sparse_len = sparse_set.sparse.len();
        let sparse: *mut Option<Box<Page>> = sparse_set.sparse.as_mut_ptr();
        let sparse = sparse as *mut *mut EntityId;

        let window = FullRawWindowMut {
            sparse,
            sparse_len,
            dense: NonNull::new(sparse_set.dense.as_mut_ptr().cast()).unwrap(),
            dense_len: sparse_set.dense.len(),
            segment_start: 0,
            data: sparse_set.data.as_mut_ptr(),
            insertion_data: sparse_set.insertion_data.as_ptr(),
            modification_data: sparse_set.modification_data.as_mut_ptr(),
            last_insertion,
            last_modification,
            current,
            is_tracking_modification: sparse_set.is_tracking_modification(),
            _phantom: PhantomData,
        };
        // The metadata points to the storage, our caller can't take it before handing us the
        // view: moving it in reborrows the storage exclusively. The window only points into
        // the storage's allocations.
        let metadata = ConstructionMetadata::window(sparse_set);

        (window, all_borrow, borrow, metadata)
    }

    #[inline]
    pub(crate) fn index_of(&self, entity: EntityId) -> Option<usize> {
        if let Some(sparse_entity) = self.sparse_index(entity) {
            if entity.generation() == sparse_entity.generation() {
                Some(sparse_entity.uindex())
            } else {
                None
            }
        } else {
            None
        }
    }

    #[inline]
    fn sparse_index(&self, entity: EntityId) -> Option<EntityId> {
        if entity.bucket() < self.sparse_len {
            // SAFE: `sparse` points to the `sparse_len` pages of the storage we borrow and
            // `bucket` is below that length. A page is an `Option<Box<Page>>`,
            // which is a nullable pointer to the page.
            let bucket = unsafe { ptr::read(self.sparse.add(entity.bucket())) };

            if !bucket.is_null() {
                // SAFE: `bucket` points to an allocated page, `Page` is `repr(C)` around its
                // `BUCKET_SIZE` entities and `bucket_index` is a remainder of `BUCKET_SIZE`.
                Some(unsafe { ptr::read(bucket.add(entity.bucket_index())) })
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns an entity source over the whole storage.
    #[inline]
    pub(crate) fn entity_iter(&self) -> RawEntityIdAccess {
        // SAFE: `dense` points to the `dense_len` entities of the storage this window borrows for `'w`.
        // The access is only used by iterators holding that borrow, which prevents any modification.
        unsafe { RawEntityIdAccess::new(self.dense, self.dense_len) }
    }

    /// Converts an index local to the current segment into an absolute storage index.
    #[inline]
    pub(crate) fn resolve_index(&self, local_index: usize) -> usize {
        self.segment_start + local_index
    }

    /// Makes the window iterate the segment starting at absolute index `segment_start`.
    ///
    /// ### Safety
    ///
    /// For every local index `i` this window will receive as a captain until the next call,
    /// `segment_start + i` must be smaller than the storage's length.\
    /// The entity source driving the iteration must hold, at local index `i`,
    /// the entity stored at absolute index `segment_start + i` in this window.
    #[inline]
    pub(crate) unsafe fn set_segment_start(&mut self, segment_start: usize) {
        self.segment_start = segment_start;
    }

    /// Appends this window's node to `row`.
    #[inline]
    pub(crate) fn describe_window(&self, active: bool, row: &mut SegmentRow) {
        row.push_window(active, self.segment_start);
    }

    /// Installs the segment start `entry` holds for this window.
    ///
    /// ### Safety
    ///
    /// `entry`'s next state must be this window's state for the segment about to be iterated.
    /// It must uphold [`set_segment_start`](Self::set_segment_start)'s requirements.
    #[inline]
    pub(crate) unsafe fn enter_window(&mut self, entry: &mut SegmentEntry<'_>) {
        let segment_start = entry.next_window();

        // SAFE: Our caller guarantees this is our segment start for the segment about to be iterated,
        // meeting `set_segment_start`'s requirements.
        unsafe { self.set_segment_start(segment_start) };
    }

    /// Returns `true` when the component at `index` was inserted in this window's tracking interval.
    ///
    /// Always `false` when `Track` doesn't track insertion.
    ///
    /// ### Safety
    ///
    /// `index` must be smaller than the storage's length.
    #[inline]
    pub(crate) unsafe fn is_inserted(&self, index: usize) -> bool {
        Track::track_insertion() && {
            // SAFE: `Track` tracks insertion and the storage was checked to track at least what
            // `Track` does, so it keeps an insertion timestamp per component.
            // Our caller guarantees `index` is one of them.
            unsafe { *self.insertion_data.add(index) }.is_within(self.last_insertion, self.current)
        }
    }

    /// Returns `true` when the component at `index` was modified in this window's tracking interval.
    ///
    /// Always `false` when `Track` doesn't track modification.
    ///
    /// ### Safety
    ///
    /// `index` must be smaller than the storage's length.
    #[inline]
    pub(crate) unsafe fn is_modified(&self, index: usize) -> bool {
        Track::track_modification() && {
            // SAFE: Same as `is_inserted`, with the modification timestamps.
            unsafe { *self.modification_data.add(index) }
                .is_within(self.last_modification, self.current)
        }
    }

    /// Returns `true` when the component at `index` was inserted or modified in this window's
    /// tracking interval, only checking the kinds `Track` tracks.
    ///
    /// ### Safety
    ///
    /// `index` must be smaller than the storage's length.
    #[inline]
    pub(crate) unsafe fn is_inserted_or_modified(&self, index: usize) -> bool {
        // SAFE: Our caller's guarantee is the one both checks need.
        unsafe { self.is_inserted(index) || self.is_modified(index) }
    }
}

impl<T: Component, Track: Tracking> Clone for FullRawWindowMut<'_, T, Track> {
    #[inline]
    fn clone(&self) -> Self {
        FullRawWindowMut {
            sparse: self.sparse,
            sparse_len: self.sparse_len,
            dense: self.dense,
            dense_len: self.dense_len,
            segment_start: self.segment_start,
            data: self.data,
            insertion_data: self.insertion_data,
            modification_data: self.modification_data,
            last_insertion: self.last_insertion,
            last_modification: self.last_modification,
            current: self.current,
            is_tracking_modification: self.is_tracking_modification,
            _phantom: PhantomData,
        }
    }
}

/// Non-owning cursor over a range of entities.
///
/// It points to the next unconsumed entity and one past the last one.\
/// It also tracks the local index of the next entity within the segment the range started at.\
/// This offset is independent from the windows' segment starts.
///
/// Empty cursors are valid, their pointers might be dangling and are never dereferenced.
#[derive(Clone, Copy)]
#[doc(hidden)]
pub struct EntityCursor {
    next: NonNull<EntityId>,
    end: NonNull<EntityId>,
    local_index: usize,
}

// SAFE: The cursor only reads entities. `new`'s caller guarantees they aren't modified while the cursor
// is used and `EntityId` is plain data, so reading them from another thread is fine.
unsafe impl Send for EntityCursor {}

impl EntityCursor {
    /// Returns a cursor without any entity.
    #[inline]
    pub(crate) fn empty() -> EntityCursor {
        EntityCursor {
            next: NonNull::dangling(),
            end: NonNull::dangling(),
            local_index: 0,
        }
    }

    /// Returns a cursor over the `len` entities starting at `start`.
    ///
    /// The first entity has local index 0.
    ///
    /// ### Safety
    ///
    /// When `len` isn't 0:
    /// - `start` must point to `len` consecutive initialized entities within a single allocation
    /// - these entities must not be deallocated or modified while this cursor or any copy of it is used
    #[inline]
    pub(crate) unsafe fn new(start: NonNull<EntityId>, len: usize) -> EntityCursor {
        if len == 0 {
            return EntityCursor::empty();
        }

        EntityCursor {
            next: start,
            // SAFE: The caller guarantees `start..start + len` is within a single allocation,
            // the result is at most one past its end.
            end: unsafe { start.add(len) },
            local_index: 0,
        }
    }

    /// Returns `true` when all entities have been consumed.
    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.next == self.end
    }

    /// Returns the number of unconsumed entities.
    #[inline]
    pub(crate) fn len(&self) -> usize {
        // SAFE: The pointers are equal, or bound a remaining range
        // within the same allocation. EntityId is not zero-sized.
        unsafe { self.end.offset_from(self.next) as usize }
    }

    /// Returns the local index of the next unconsumed entity.
    #[inline(always)]
    pub(crate) fn local_index(&self) -> usize {
        self.local_index
    }

    /// Returns a pointer to the next unconsumed entity.
    ///
    /// The pointer might be dangling when the cursor is empty.
    #[inline]
    pub(crate) fn as_ptr(&self) -> NonNull<EntityId> {
        self.next
    }

    /// Returns the next unconsumed entity.
    ///
    /// ### Safety
    ///
    /// The cursor must not be empty.
    #[inline(always)]
    pub(crate) unsafe fn current(&self) -> EntityId {
        debug_assert!(!self.is_empty());

        // SAFE: The caller guarantees the cursor isn't empty so `next` points to an initialized
        // entity that `new`'s caller keeps alive.
        unsafe { self.next.read() }
    }

    /// Consumes the next entity.
    ///
    /// Returns `false` without moving when the cursor is empty.
    #[inline(always)]
    pub(crate) fn advance(&mut self) -> bool {
        if self.is_empty() {
            return false;
        }

        // SAFE: The cursor isn't empty so `next < end` and both are within the same allocation.
        // The result is at most `end`.
        self.next = unsafe { self.next.add(1) };
        self.local_index += 1;

        true
    }

    /// Splits the unconsumed entities after the first `index` of them.
    ///
    /// Both halves keep the local indices of the original range, either can be empty.
    ///
    /// ### Panics
    ///
    /// Panics when `index` is greater than the number of unconsumed entities.
    #[inline]
    pub(crate) fn split_at(self, index: usize) -> (EntityCursor, EntityCursor) {
        assert!(index <= self.len(), "split index out of bounds");

        // SAFE: `index <= len` so the result is within `next..=end`.
        let middle = unsafe { self.next.add(index) };

        let left = EntityCursor {
            next: self.next,
            end: middle,
            local_index: self.local_index,
        };
        let right = EntityCursor {
            next: middle,
            end: self.end,
            local_index: self.local_index + index,
        };

        (left, right)
    }

    /// Returns a cursor over `len` unconsumed entities, after skipping the first `start` of them.
    ///
    /// The new cursor starts a segment: its first entity has local index 0.
    ///
    /// ### Panics
    ///
    /// Panics when the range goes past the unconsumed entities.
    #[inline]
    pub(crate) fn segment(self, start: usize, len: usize) -> EntityCursor {
        let end = start.checked_add(len);
        assert!(
            end.is_some_and(|end| end <= self.len()),
            "segment out of bounds"
        );

        let (_, rest) = self.split_at(start);
        let (segment, _) = rest.split_at(len);

        EntityCursor {
            local_index: 0,
            ..segment
        }
    }
}
