use crate::atomic_refcell::{ExclusiveBorrow, SharedBorrow};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::{RawEntityIdAccess, SegmentEntry, SegmentRow};
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
pub struct FullRawWindow<'a, T> {
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
    _phantom: PhantomData<&'a T>,
}

unsafe impl<T: Send + Component> Send for FullRawWindow<'_, T> {}

impl<'w, T: Component> FullRawWindow<'w, T> {
    #[inline]
    pub(crate) fn from_view<Track: Tracking>(view: &View<'_, T, Track>) -> Self {
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
    pub(crate) fn from_owned_view<Track: Tracking>(
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
    pub(crate) fn from_view_mut<Track: Tracking>(view: &ViewMut<'_, T, Track>) -> Self {
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
            if entity.gen() == sparse_entity.gen() {
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
            let bucket = unsafe { ptr::read(self.sparse.add(entity.bucket())) };

            if !bucket.is_null() {
                Some(unsafe { ptr::read(bucket.add(entity.bucket_index())) })
            } else {
                None
            }
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.dense_len
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
}

impl<T: Component> Clone for FullRawWindow<'_, T> {
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
pub struct FullRawWindowMut<'a, T, Track> {
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

unsafe impl<T: Send + Component, Track> Send for FullRawWindowMut<'_, T, Track> {}

impl<'w, T: Component, Track> FullRawWindowMut<'w, T, Track> {
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

    #[inline]
    pub(crate) fn new_owned<TRACK>(
        view: ViewMut<'_, T, TRACK>,
    ) -> (Self, Option<SharedBorrow<'_>>, ExclusiveBorrow<'_>) {
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

        (
            FullRawWindowMut {
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
            },
            all_borrow,
            borrow,
        )
    }

    #[inline]
    pub(crate) fn index_of(&self, entity: EntityId) -> Option<usize> {
        if let Some(sparse_entity) = self.sparse_index(entity) {
            if entity.gen() == sparse_entity.gen() {
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
            let bucket = unsafe { ptr::read(self.sparse.add(entity.bucket())) };

            if !bucket.is_null() {
                Some(unsafe { ptr::read(bucket.add(entity.bucket_index())) })
            } else {
                None
            }
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.dense_len
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
}

impl<T: Component, Track> Clone for FullRawWindowMut<'_, T, Track> {
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

// The cursor only reads entities. `new`'s caller guarantees they aren't modified while the cursor
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
    #[inline]
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
    #[inline]
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
    #[inline]
    pub(crate) unsafe fn current(&self) -> EntityId {
        debug_assert!(!self.is_empty());

        // SAFE: The caller guarantees the cursor isn't empty so `next` points to an initialized
        // entity that `new`'s caller keeps alive.
        unsafe { self.next.read() }
    }

    /// Consumes the next entity.
    ///
    /// Returns `false` without moving when the cursor is empty.
    #[inline]
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

#[cfg(test)]
mod grouping_cursor {
    use super::EntityCursor;
    use crate::EntityId;
    use alloc::vec::Vec;
    use core::ptr::NonNull;

    fn entities(len: u64) -> Vec<EntityId> {
        (0..len).map(EntityId::new).collect()
    }

    fn cursor(entities: &[EntityId]) -> EntityCursor {
        // SAFE: `entities` is a live slice that outlives every use of the cursor in these tests
        // and isn't modified meanwhile.
        unsafe { EntityCursor::new(NonNull::from(entities).cast(), entities.len()) }
    }

    /// Asserts that empty cursors report no entity and never move,
    /// whether built empty, from a dangling pointer or from an empty slice.
    #[test]
    fn empty() {
        // SAFE: `len` is 0 so the pointer is never read.
        let from_dangling = unsafe { EntityCursor::new(NonNull::dangling(), 0) };
        let from_slice = cursor(&[]);

        for mut cursor in [EntityCursor::empty(), from_dangling, from_slice] {
            assert!(cursor.is_empty());
            assert_eq!(cursor.len(), 0);
            assert!(!cursor.advance());
            assert_eq!(cursor.local_index(), 0);
        }
    }

    /// Asserts that a single entity cursor yields its entity once then stays exhausted.
    #[test]
    fn one() {
        let entities = entities(1);
        let mut cursor = cursor(&entities);

        assert_eq!(cursor.len(), 1);
        // SAFE: The cursor holds one entity.
        assert_eq!(unsafe { cursor.current() }, entities[0]);

        assert!(cursor.advance());
        assert!(cursor.is_empty());
        assert_eq!(cursor.local_index(), 1);
    }

    /// Asserts that advancing an exhausted cursor repeatedly doesn't move it.
    #[test]
    fn exhausted_advance() {
        let entities = entities(2);
        let mut cursor = cursor(&entities);

        assert!(cursor.advance());
        assert!(cursor.advance());
        for _ in 0..3 {
            assert!(!cursor.advance());
            assert_eq!(cursor.len(), 0);
            assert_eq!(cursor.local_index(), 2);
        }
    }

    /// Asserts that a partially consumed cursor reads the next unconsumed entity
    /// and reports its local index and remaining length.
    #[test]
    fn partial() {
        let entities = entities(5);
        let mut cursor = cursor(&entities);

        cursor.advance();
        cursor.advance();

        assert_eq!(cursor.len(), 3);
        assert_eq!(cursor.local_index(), 2);
        // SAFE: 3 entities are left.
        assert_eq!(unsafe { cursor.current() }, entities[2]);
    }

    /// Asserts that splitting a range whose next local index is 3 keeps the original
    /// local indices on both halves instead of restarting the right one at zero.
    #[test]
    fn split_offsets() {
        let entities = entities(8);
        let mut cursor = cursor(&entities);
        for _ in 0..3 {
            cursor.advance();
        }

        let (left, right) = cursor.split_at(2);

        assert_eq!((left.local_index(), left.len()), (3, 2));
        assert_eq!((right.local_index(), right.len()), (5, 3));
        // SAFE: Both halves are nonempty.
        unsafe {
            assert_eq!(left.current(), entities[3]);
            assert_eq!(right.current(), entities[5]);
        }
    }

    /// Asserts that splitting at either end gives an empty half and a half with every entity,
    /// both keeping the right local indices.
    #[test]
    fn split_ends() {
        let entities = entities(3);
        let mut cursor = cursor(&entities);
        cursor.advance();

        let (left, right) = cursor.split_at(0);
        assert_eq!((left.local_index(), left.len()), (1, 0));
        assert_eq!((right.local_index(), right.len()), (1, 2));

        let (left, right) = cursor.split_at(2);
        assert_eq!((left.local_index(), left.len()), (1, 2));
        assert_eq!((right.local_index(), right.len()), (3, 0));
        assert!(right.is_empty());
    }

    /// Asserts that splitting past the unconsumed entities panics instead of creating
    /// an out of bounds cursor.
    #[test]
    #[should_panic(expected = "split index out of bounds")]
    fn split_out_of_bounds() {
        let entities = entities(3);
        let mut cursor = cursor(&entities);
        cursor.advance();

        let _ = cursor.split_at(3);
    }

    /// Asserts that a segment taken from a partially consumed cursor starts after the
    /// skipped entities and restarts local indices at 0.
    #[test]
    fn segment() {
        let entities = entities(6);
        let mut cursor = cursor(&entities);
        cursor.advance();

        let segment = cursor.segment(2, 3);

        assert_eq!((segment.local_index(), segment.len()), (0, 3));
        // SAFE: The segment holds 3 entities.
        assert_eq!(unsafe { segment.current() }, entities[3]);
    }

    /// Asserts that a segment ending past the unconsumed entities panics instead of creating
    /// an out of bounds cursor.
    #[test]
    #[should_panic(expected = "segment out of bounds")]
    fn segment_out_of_bounds() {
        let entities = entities(4);

        let _ = cursor(&entities).segment(2, 3);
    }

    /// Asserts that a segment whose end overflows panics instead of wrapping.
    #[test]
    #[should_panic(expected = "segment out of bounds")]
    fn segment_overflow() {
        let entities = entities(4);

        let _ = cursor(&entities).segment(1, usize::MAX);
    }
}

#[cfg(test)]
mod grouping_coordinates {
    use super::{EntityCursor, FullRawWindow, FullRawWindowMut};
    use crate::iter::{Mixed, Shiper, Shiperator};
    use crate::tracking::Modified;
    use crate::{track, Component, EntityId, Get, View, ViewMut, World};
    use alloc::vec::Vec;
    use core::ptr::NonNull;

    #[derive(Debug, PartialEq)]
    struct A(u32);
    impl Component for A {
        type Tracking = track::Untracked;
    }

    #[derive(Debug, PartialEq)]
    struct B(u32);
    impl Component for B {
        type Tracking = track::Untracked;
    }

    #[derive(Debug, PartialEq)]
    struct Tracked(u32);
    impl Component for Tracked {
        type Tracking = track::Modification;
    }

    #[derive(Debug, PartialEq)]
    struct Zst;
    impl Component for Zst {
        type Tracking = track::Untracked;
    }

    /// Builds six entities where entity `i` owns `A(i)` at index `i` of `A`.
    /// Entities 2 to 5 also own `B(10 + i)` at index `i - 1` of `B`,
    /// after an unrelated `B(100)` at index 0.
    fn world() -> (World, Vec<EntityId>) {
        let mut world = World::new();

        world.add_entity(B(100));
        let entities: Vec<EntityId> = (0..6).map(|i| world.add_entity(A(i))).collect();
        for (i, &entity) in entities.iter().enumerate().skip(2) {
            world.add_component(entity, B(10 + i as u32));
        }

        (world, entities)
    }

    /// Returns a cursor over the `len` entities starting at index `start` of `dense`.
    fn entities_from(dense: &[EntityId], start: usize, len: usize) -> EntityCursor {
        let segment = &dense[start..start + len];

        // SAFE: The storage `dense` belongs to stays borrowed while the tests use the cursor.
        unsafe { EntityCursor::new(NonNull::from(segment).cast(), len) }
    }

    /// Iterates `len` items of `shiperator`, reading entities from `dense` starting at `start`.
    ///
    /// Segment starts must be set on `shiperator`'s windows beforehand.
    fn segment_iter<S: Shiperator>(
        shiperator: S,
        dense: &[EntityId],
        start: usize,
        len: usize,
    ) -> Shiper<S> {
        let is_exact_sized = shiperator.is_exact_sized();

        Shiper {
            shiperator,
            cursor: entities_from(dense, start, len),
            schedule: None,
            is_exact_sized,
        }
    }

    /// Asserts that two windows with different nonzero segment starts resolve the same
    /// local index to the components of the same entity, adding their start only once.
    #[test]
    fn local_indices() {
        let (world, _) = world();
        let a = world.borrow::<View<'_, A>>().unwrap();
        let b = world.borrow::<View<'_, B>>().unwrap();
        let mut entities = entities_from(&a.dense, 2, 4);

        let mut a_window = FullRawWindow::from_view(&a);
        let mut b_window = FullRawWindow::from_view(&b);
        // SAFE: Both segments are 4 components long and cover entities 2 to 5,
        // stored at 2..6 in A, at 1..5 in B and at 0..4 in `entities`.
        unsafe {
            a_window.set_segment_start(2);
            b_window.set_segment_start(1);
        }

        for local in 0..4 {
            assert_eq!(entities.local_index(), local);

            // SAFE: `local` is within the 4 components long segments
            // and is the local index of the current candidate of `entities`.
            let (a_component, b_component) = unsafe {
                let a_index = a_window.captain_indices_of(&entities, local).unwrap();
                let b_index = b_window.captain_indices_of(&entities, local).unwrap();

                (a_window.get_data(a_index), b_window.get_data(b_index))
            };

            assert_eq!(a_component, &A(2 + local as u32));
            assert_eq!(b_component, &B(12 + local as u32));

            entities.advance();
        }
    }

    /// Asserts that `Mixed` forwards local indices unchanged to multiple captains,
    /// each applying its own start, and that `WithId` yields the matching entities.
    #[test]
    fn mixed_captains() {
        let (world, entities) = world();
        let a = world.borrow::<View<'_, A>>().unwrap();
        let b = world.borrow::<View<'_, B>>().unwrap();

        let mut a_window = FullRawWindow::from_view(&a);
        let mut b_window = FullRawWindow::from_view(&b);
        // SAFE: Both segments are 4 components long and cover entities 2 to 5,
        // stored at 2..6 in A, at 1..5 in B and at 0..4 in the iterator's entity source.
        unsafe {
            a_window.set_segment_start(2);
            b_window.set_segment_start(1);
        }

        let mixed = Mixed::new((a_window, b_window), 0b11);
        let iter = segment_iter(mixed, &a.dense, 2, 4);

        let items: Vec<_> = iter.with_id().map(|(id, (a, b))| (id, a.0, b.0)).collect();
        let expected: Vec<_> = (2..6)
            .map(|i| (entities[i], i as u32, 10 + i as u32))
            .collect();
        assert_eq!(items, expected);
    }

    /// Asserts that sparse lookups ignore the segment start and return absolute indices,
    /// including for an entity stored before the segment.
    #[test]
    fn sailor_absolute() {
        let (world, entities) = world();
        let a = world.borrow::<View<'_, A>>().unwrap();

        let mut a_window = FullRawWindow::from_view(&a);
        // SAFE: The segment covers entities 4 and 5, stored at 4..6 in A.
        // The window is never used as a captain.
        unsafe {
            a_window.set_segment_start(4);
        }

        let index = a_window.sailor_indices_of(entities[1]).unwrap();
        assert_eq!(index, 1);
        // SAFE: `index` was returned by `sailor_indices_of`.
        assert_eq!(unsafe { a_window.get_data(index) }, &A(1));
    }

    /// Asserts that a sailor with a nonzero segment start returns the components of
    /// the captain's entities, even the ones stored before its own segment.
    #[test]
    fn nested_sailor() {
        let (world, entities) = world();
        let a = world.borrow::<View<'_, A>>().unwrap();
        let b = world.borrow::<View<'_, B>>().unwrap();

        let mut a_window = FullRawWindow::from_view(&a);
        let mut b_window = FullRawWindow::from_view(&b);
        // SAFE: A is only a sailor, its segment start is never applied.
        // B's segment is 4 components long and covers entities 2 to 5,
        // stored at 1..5 in B and at 0..4 in the iterator's entity source.
        unsafe {
            a_window.set_segment_start(4);
            b_window.set_segment_start(1);
        }

        let mixed = Mixed::new((a_window, b_window), 0b10);
        let iter = segment_iter(mixed, &b.dense, 1, 4);

        let items: Vec<_> = iter.with_id().map(|(id, (a, b))| (id, a.0, b.0)).collect();
        let expected: Vec<_> = (2..6)
            .map(|i| (entities[i], i as u32, 10 + i as u32))
            .collect();
        assert_eq!(items, expected);
    }

    /// Asserts that resolving indices and creating `Mut`s without writing through them
    /// doesn't mark any component as modified.
    #[test]
    fn resolve_no_modify() {
        let mut world = World::new();
        let entities: Vec<EntityId> = (0..4).map(|i| world.add_entity(Tracked(i))).collect();
        let mut tracked = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        let mut segment_entities = entities_from(&tracked.dense, 1, 3);

        let mut window = FullRawWindowMut::new(&mut tracked);
        // SAFE: The segment is 3 components long and covers entities 1 to 3,
        // stored at 1..4 in the storage and at 0..3 in `segment_entities`.
        unsafe {
            window.set_segment_start(1);
        }

        for local in 0..3 {
            // SAFE: `local` is within the 3 components long segment
            // and is the local index of the current candidate of `segment_entities`.
            // Each `Mut` is dropped before the next one is created.
            unsafe {
                let index = window.captain_indices_of(&segment_entities, local).unwrap();
                let _ = window.get_data(index);
            }

            segment_entities.advance();
        }

        for entity in entities {
            assert!(!tracked.is_modified(entity));
        }
    }

    /// Asserts that a tracked window writes and reads the modification timestamp
    /// of the resolved component, not the one at the unadjusted local index.
    #[test]
    fn tracked_resolution() {
        let mut world = World::new();
        let entities: Vec<EntityId> = (0..4).map(|i| world.add_entity(Tracked(i))).collect();
        let mut tracked = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        let mut segment_entities = entities_from(&tracked.dense, 1, 3);

        let mut window = FullRawWindowMut::new(&mut tracked);
        // SAFE: The segment is 3 components long and covers entities 1 to 3,
        // stored at 1..4 in the storage and at 0..3 in `segment_entities`.
        unsafe {
            window.set_segment_start(1);
        }

        segment_entities.advance();
        // SAFE: Local index 1 is within the 3 components long segment
        // and is the local index of the current candidate of `segment_entities`.
        // No other reference to the component exists while `component` lives.
        unsafe {
            let index = window.captain_indices_of(&segment_entities, 1).unwrap();
            let mut component = window.get_data(index);
            component.0 = 42;
        }

        let modified = Modified(window);
        let mut segment_entities = entities_from(&tracked.dense, 1, 3);
        let modified_indices: Vec<_> = (0..3)
            .map(|local| {
                // SAFE: `local` is within the 3 components long segment
                // and is the local index of the current candidate of `segment_entities`.
                let index = unsafe { modified.captain_indices_of(&segment_entities, local) };
                segment_entities.advance();

                index
            })
            .collect();
        assert_eq!(modified_indices, [None, Some(2), None]);

        assert_eq!((&tracked).get(entities[2]).unwrap(), &Tracked(42));
        assert!(tracked.is_modified(entities[2]));
        assert!(!tracked.is_modified(entities[1]));
    }

    /// Asserts that iterating an empty storage yields nothing.
    #[test]
    fn empty_storage() {
        let world = World::new();
        let a = world.borrow::<View<'_, A>>().unwrap();

        let window = FullRawWindow::from_view(&a);
        let iter = segment_iter(window, &a.dense, 0, 0);

        assert_eq!(iter.count(), 0);
    }

    /// Asserts that a zero-sized component's window resolves local indices
    /// to the entities of its segment.
    #[test]
    fn zero_sized() {
        let mut world = World::new();
        let entities: Vec<EntityId> = (0..5).map(|_| world.add_entity(Zst)).collect();
        let zsts = world.borrow::<View<'_, Zst>>().unwrap();

        let mut window = FullRawWindow::from_view(&zsts);
        // SAFE: The segment covers entities 2 to 4, stored at 2..5 in the storage and at 0..3
        // in the iterator's entity source.
        unsafe {
            window.set_segment_start(2);
        }
        let iter = segment_iter(window, &zsts.dense, 2, 3);

        let items: Vec<_> = iter.with_id().map(|(id, _)| id).collect();
        assert_eq!(items, &entities[2..]);
    }

    /// Asserts that cloning a window keeps its segment start.
    #[test]
    fn clone_keeps_start() {
        let (world, entities) = world();
        let a = world.borrow::<View<'_, A>>().unwrap();

        let mut window = FullRawWindow::from_view(&a);
        // SAFE: The segment covers entities 2 to 5, stored at 2..6 in A and at 0..4 in
        // the iterator's entity source.
        unsafe {
            window.set_segment_start(2);
        }

        let iter = segment_iter(window.clone(), &a.dense, 2, 4);

        let items: Vec<_> = iter.with_id().map(|(id, a)| (id, a.0)).collect();
        let expected: Vec<_> = (2..6).map(|i| (entities[i], i as u32)).collect();
        assert_eq!(items, expected);
    }
}
