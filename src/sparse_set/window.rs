use crate::atomic_refcell::{ExclusiveBorrow, SharedBorrow};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::tracking::{Tracking, TrackingTimestamp};
use crate::views::{View, ViewMut};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ptr::{self, NonNull};

pub struct FullRawWindow<'a, T> {
    sparse: *const *const EntityId,
    sparse_len: usize,
    pub(crate) dense: NonNull<EntityId>,
    pub(crate) dense_len: usize,
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
        let sparse: *const Option<Box<[EntityId; super::BUCKET_SIZE]>> = view.sparse.as_ptr();
        let sparse = sparse as *const *const EntityId;

        FullRawWindow {
            sparse,
            sparse_len,
            dense: NonNull::new(view.dense.as_ptr().cast_mut()).unwrap(),
            dense_len: view.dense.len(),
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

        let sparse_len = sparse_set.len();
        let sparse: *const Option<Box<[EntityId; super::BUCKET_SIZE]>> = sparse_set.sparse.as_ptr();
        let sparse = sparse as *const *const EntityId;

        (
            FullRawWindow {
                sparse,
                sparse_len,
                dense: NonNull::new(sparse_set.dense.as_ptr().cast_mut()).unwrap(),
                dense_len: sparse_set.dense.len(),
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
        let sparse: *const Option<Box<[EntityId; super::BUCKET_SIZE]>> = view.sparse.as_ptr();
        let sparse = sparse as *const *const EntityId;

        FullRawWindow {
            sparse,
            sparse_len,
            dense: NonNull::new(view.dense.as_ptr().cast_mut()).unwrap(),
            dense_len: view.dense.len(),
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
        self.sparse_index(entity).and_then(|sparse_entity| {
            if entity.gen() == sparse_entity.gen() {
                Some(sparse_entity.uindex())
            } else {
                None
            }
        })
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
}

impl<T: Component> Clone for FullRawWindow<'_, T> {
    #[inline]
    fn clone(&self) -> Self {
        FullRawWindow {
            sparse: self.sparse,
            sparse_len: self.sparse_len,
            dense: self.dense,
            dense_len: self.dense_len,
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

pub struct FullRawWindowMut<'a, T, Track> {
    sparse: *mut *mut EntityId,
    sparse_len: usize,
    pub(crate) dense: NonNull<EntityId>,
    pub(crate) dense_len: usize,
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
        let sparse: *mut Option<Box<[EntityId; super::BUCKET_SIZE]>> = view.sparse.as_mut_ptr();
        let sparse = sparse as *mut *mut EntityId;

        FullRawWindowMut {
            sparse,
            sparse_len,
            dense: NonNull::new(view.dense.as_mut_ptr().cast()).unwrap(),
            dense_len: view.dense.len(),
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

        let sparse_len = sparse_set.len();
        let sparse: *mut Option<Box<[EntityId; super::BUCKET_SIZE]>> =
            sparse_set.sparse.as_mut_ptr();
        let sparse = sparse as *mut *mut EntityId;

        (
            FullRawWindowMut {
                sparse,
                sparse_len,
                dense: NonNull::new(sparse_set.dense.as_mut_ptr().cast()).unwrap(),
                dense_len: sparse_set.dense.len(),
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
        self.sparse_index(entity).and_then(|sparse_entity| {
            if entity.gen() == sparse_entity.gen() {
                Some(sparse_entity.uindex())
            } else {
                None
            }
        })
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
}

impl<T: Component, Track> Clone for FullRawWindowMut<'_, T, Track> {
    #[inline]
    fn clone(&self) -> Self {
        FullRawWindowMut {
            sparse: self.sparse,
            sparse_len: self.sparse_len,
            dense: self.dense,
            dense_len: self.dense_len,
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
    pub ptr: NonNull<EntityId>,
    pub follow_up_ptrs: Vec<(NonNull<EntityId>, usize)>,
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
