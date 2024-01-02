use crate::atomic_refcell::{ExclusiveBorrow, SharedBorrow};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::tracking::TrackingTimestamp;
use crate::views::{View, ViewMut};
use alloc::boxed::Box;
use core::hint::unreachable_unchecked;
use core::marker::PhantomData;
use core::ptr;

pub struct FullRawWindow<'a, T> {
    sparse: *const *const EntityId,
    sparse_len: usize,
    pub(crate) dense: *const EntityId,
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
    pub(crate) fn from_view<TRACK>(view: &View<'_, T, TRACK>) -> Self {
        let sparse_len = view.sparse.len();
        let sparse: *const Option<Box<[EntityId; super::BUCKET_SIZE]>> = view.sparse.as_ptr();
        let sparse = sparse as *const *const EntityId;

        FullRawWindow {
            sparse,
            sparse_len,
            dense: view.dense.as_ptr(),
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
    pub(crate) fn from_owned_view<TRACK>(
        view: View<'_, T, TRACK>,
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
                dense: sparse_set.dense.as_ptr(),
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
    pub(crate) fn from_view_mut<TRACK>(view: &ViewMut<'_, T, TRACK>) -> Self {
        let sparse_len = view.sparse.len();
        let sparse: *const Option<Box<[EntityId; super::BUCKET_SIZE]>> = view.sparse.as_ptr();
        let sparse = sparse as *const *const EntityId;

        FullRawWindow {
            sparse,
            sparse_len,
            dense: view.dense.as_ptr(),
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
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this window and until a modification happens.
    /// # Safety
    ///
    /// `entity` has to own a component in this storage.  
    /// In case it used to but no longer does, the result will be wrong but won't trigger any UB.
    #[inline]
    pub(crate) unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
        if let Some(index) = self.index_of(entity) {
            index
        } else {
            unreachable_unchecked()
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

pub struct FullRawWindowMut<'a, T> {
    sparse: *mut *mut EntityId,
    sparse_len: usize,
    pub(crate) dense: *mut EntityId,
    pub(crate) dense_len: usize,
    pub(crate) data: *mut T,
    pub(crate) insertion_data: *const TrackingTimestamp,
    pub(crate) modification_data: *mut TrackingTimestamp,
    pub(crate) last_insertion: TrackingTimestamp,
    pub(crate) last_modification: TrackingTimestamp,
    pub(crate) current: TrackingTimestamp,
    pub(crate) is_tracking_modification: bool,
    _phantom: PhantomData<&'a mut T>,
}

unsafe impl<T: Send + Component> Send for FullRawWindowMut<'_, T> {}

impl<'w, T: Component> FullRawWindowMut<'w, T> {
    #[inline]
    pub(crate) fn new<TRACK>(view: &mut ViewMut<'_, T, TRACK>) -> Self {
        let sparse_len = view.sparse.len();
        let sparse: *mut Option<Box<[EntityId; super::BUCKET_SIZE]>> = view.sparse.as_mut_ptr();
        let sparse = sparse as *mut *mut EntityId;

        FullRawWindowMut {
            sparse,
            sparse_len,
            dense: view.dense.as_mut_ptr(),
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
                dense: sparse_set.dense.as_mut_ptr(),
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
    /// Returns the index of `entity`'s component in the `dense` and `data` vectors.  
    /// This index is only valid for this window and until a modification happens.
    /// # Safety
    ///
    /// `entity` has to own a component in this storage.  
    /// In case it used to but no longer does, the result will be wrong but won't trigger any UB.
    #[inline]
    pub(crate) unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
        if let Some(index) = self.index_of(entity) {
            index
        } else {
            unreachable_unchecked()
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
}

impl<T: Component> Clone for FullRawWindowMut<'_, T> {
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
