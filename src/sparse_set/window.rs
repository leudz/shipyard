use crate::atomic_refcell::{ExclusiveBorrow, SharedBorrow};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::sparse_array::Page;
use crate::tracking::{Tracking, TrackingTimestamp};
use crate::views::{View, ViewMut};
use alloc::boxed::Box;
use alloc::vec::Vec;
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

    #[inline]
    pub(crate) fn entity_iter(&self) -> RawEntityIdAccess {
        RawEntityIdAccess::new(self.dense, Vec::new())
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
    #[allow(dead_code, reason = "used by coordinated segment entry")]
    pub(crate) unsafe fn set_segment_start(&mut self, segment_start: usize) {
        self.segment_start = segment_start;
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

    #[inline]
    pub(crate) fn entity_iter(&self) -> RawEntityIdAccess {
        RawEntityIdAccess::new(self.dense, Vec::new())
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
    #[allow(dead_code, reason = "used by coordinated segment entry")]
    pub(crate) unsafe fn set_segment_start(&mut self, segment_start: usize) {
        self.segment_start = segment_start;
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

#[derive(Clone)]
#[doc(hidden)]
pub struct RawEntityIdAccess {
    ptr: NonNull<EntityId>,
    follow_up_ptrs: Vec<(NonNull<EntityId>, usize)>,
}

unsafe impl Send for RawEntityIdAccess {}

impl RawEntityIdAccess {
    #[inline]
    #[doc(hidden)]
    pub fn new(
        ptr: NonNull<EntityId>,
        follow_up_ptrs: Vec<(NonNull<EntityId>, usize)>,
    ) -> RawEntityIdAccess {
        RawEntityIdAccess {
            ptr,
            follow_up_ptrs,
        }
    }

    #[inline]
    pub fn ptr(&self) -> NonNull<EntityId> {
        self.ptr
    }

    #[inline]
    #[doc(hidden)]
    pub fn dangling() -> RawEntityIdAccess {
        RawEntityIdAccess {
            ptr: NonNull::dangling(),
            follow_up_ptrs: Vec::new(),
        }
    }

    #[inline]
    #[doc(hidden)]
    pub unsafe fn get(&self, index: usize) -> EntityId {
        self.ptr.add(index).read()
    }

    #[inline]
    #[doc(hidden)]
    pub fn follow_up_len(&self) -> usize {
        self.follow_up_ptrs
            .iter()
            .map(|(_, len)| len)
            .sum::<usize>()
    }

    #[inline]
    #[doc(hidden)]
    pub fn next_slice(&mut self) -> Option<usize> {
        let Some((new_start, new_end)) = self.follow_up_ptrs.pop() else {
            return None;
        };

        self.ptr = new_start;

        Some(new_end)
    }

    #[inline]
    #[doc(hidden)]
    pub fn split_at(mut self, count: usize) -> (RawEntityIdAccess, RawEntityIdAccess) {
        let mut total_len = 0;
        let mut remaining = 0;
        let split_point = self.follow_up_ptrs.iter().position(|(_, len)| {
            total_len += len;

            if total_len >= count {
                remaining = count - (total_len - len);

                true
            } else {
                false
            }
        });

        let mut follow_up2 = if let Some(split_point) = split_point {
            self.follow_up_ptrs.split_off(split_point)
        } else {
            core::mem::take(&mut self.follow_up_ptrs)
        };

        if remaining != 0 {
            let right = &mut follow_up2[0];
            self.follow_up_ptrs.push((right.0, remaining));
            right.1 -= remaining;
        }

        let other = RawEntityIdAccess {
            ptr: self.ptr,
            follow_up_ptrs: follow_up2,
        };

        (self, other)
    }
}

#[cfg(test)]
mod grouping_coordinates {
    use super::{FullRawWindow, FullRawWindowMut, RawEntityIdAccess};
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

    /// Returns an entity source starting at index `start` of `dense`.
    fn entities_from(dense: &[EntityId], start: usize) -> RawEntityIdAccess {
        RawEntityIdAccess::new(NonNull::from(&dense[start..]).cast(), Vec::new())
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
            entities: entities_from(dense, start),
            is_exact_sized,
            start: 0,
            end: len,
        }
    }

    /// Asserts that two windows with different nonzero segment starts resolve the same
    /// local index to the components of the same entity, adding their start only once.
    #[test]
    fn local_indices() {
        let (world, _) = world();
        let a = world.borrow::<View<'_, A>>().unwrap();
        let b = world.borrow::<View<'_, B>>().unwrap();
        let mut entities = entities_from(&a.dense, 2);

        let mut a_window = FullRawWindow::from_view(&a);
        let mut b_window = FullRawWindow::from_view(&b);
        // SAFE: Both segments are 4 components long and cover entities 2 to 5,
        // stored at 2..6 in A, at 1..5 in B and at 0..4 in `entities`.
        unsafe {
            a_window.set_segment_start(2);
            b_window.set_segment_start(1);
        }

        for local in 0..4 {
            // SAFE: `local` is within the 4 components long segments.
            let (a_component, b_component) = unsafe {
                let a_index = a_window.captain_indices_of(&mut entities, local).unwrap();
                let b_index = b_window.captain_indices_of(&mut entities, local).unwrap();

                (a_window.get_data(a_index), b_window.get_data(b_index))
            };

            assert_eq!(a_component, &A(2 + local as u32));
            assert_eq!(b_component, &B(12 + local as u32));
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

        let mixed = Mixed {
            shiperator: (a_window, b_window),
            captain_mask: 0b11,
        };
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

        let mixed = Mixed {
            shiperator: (a_window, b_window),
            captain_mask: 0b10,
        };
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
        let mut segment_entities = entities_from(&tracked.dense, 1);

        let mut window = FullRawWindowMut::new(&mut tracked);
        // SAFE: The segment is 3 components long and covers entities 1 to 3,
        // stored at 1..4 in the storage and at 0..3 in `segment_entities`.
        unsafe {
            window.set_segment_start(1);
        }

        for local in 0..3 {
            // SAFE: `local` is within the 3 components long segment.
            // Each `Mut` is dropped before the next one is created.
            unsafe {
                let index = window
                    .captain_indices_of(&mut segment_entities, local)
                    .unwrap();
                let _ = window.get_data(index);
            }
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
        let mut segment_entities = entities_from(&tracked.dense, 1);

        let mut window = FullRawWindowMut::new(&mut tracked);
        // SAFE: The segment is 3 components long and covers entities 1 to 3,
        // stored at 1..4 in the storage and at 0..3 in `segment_entities`.
        unsafe {
            window.set_segment_start(1);
        }

        // SAFE: Local index 1 is within the 3 components long segment.
        // No other reference to the component exists while `component` lives.
        unsafe {
            let index = window.captain_indices_of(&mut segment_entities, 1).unwrap();
            let mut component = window.get_data(index);
            component.0 = 42;
        }

        let modified = Modified(window);
        // SAFE: Local indices 0 to 2 are within the 3 components long segment.
        let modified_indices: Vec<_> = (0..3)
            .map(|local| unsafe { modified.captain_indices_of(&mut segment_entities, local) })
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
