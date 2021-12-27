use crate::component::Component;
use crate::view::{View, ViewMut};
use crate::EntityId;
use alloc::boxed::Box;
use core::hint::unreachable_unchecked;
use core::marker::PhantomData;
use core::ptr;

pub struct FullRawWindow<'a, T, Tracking> {
    sparse: *const *const EntityId,
    sparse_len: usize,
    pub(crate) dense: *const EntityId,
    pub(crate) dense_len: usize,
    pub(crate) data: *const T,
    pub(crate) insertion_data: *const u32,
    pub(crate) modification_data: *const u32,
    pub(crate) last_insertion: u32,
    pub(crate) last_modification: u32,
    pub(crate) current: u32,
    pub(crate) is_tracking_modification: bool,
    _phantom: PhantomData<(&'a T, Tracking)>,
}

unsafe impl<T: Send + Component> Send for FullRawWindow<'_, T, T::Tracking> {}

impl<'w, T: Component> FullRawWindow<'w, T, T::Tracking> {
    #[inline]
    pub(crate) fn from_view(sparse_set: &View<'_, T, T::Tracking>) -> Self {
        let sparse_len = sparse_set.sparse.len();
        let sparse: *const Option<Box<[EntityId; super::BUCKET_SIZE]>> = sparse_set.sparse.as_ptr();
        let sparse = sparse as *const *const EntityId;
        let is_tracking_modification = sparse_set.is_tracking_modification();

        FullRawWindow {
            sparse,
            sparse_len,
            dense: sparse_set.dense.as_ptr(),
            dense_len: sparse_set.dense.len(),
            data: sparse_set.data.as_ptr(),
            insertion_data: sparse_set.insertion_data.as_ptr(),
            modification_data: sparse_set.modification_data.as_ptr(),
            last_insertion: sparse_set.last_insert,
            last_modification: sparse_set.last_modification,
            current: sparse_set.current,
            is_tracking_modification,
            _phantom: PhantomData,
        }
    }
    #[inline]
    pub(crate) fn from_view_mut(sparse_set: &ViewMut<'_, T, T::Tracking>) -> Self {
        let sparse_len = sparse_set.sparse.len();
        let sparse: *const Option<Box<[EntityId; super::BUCKET_SIZE]>> = sparse_set.sparse.as_ptr();
        let sparse = sparse as *const *const EntityId;
        let is_tracking_modification = sparse_set.is_tracking_modification();

        FullRawWindow {
            sparse,
            sparse_len,
            dense: sparse_set.dense.as_ptr(),
            dense_len: sparse_set.dense.len(),
            data: sparse_set.data.as_ptr(),
            insertion_data: sparse_set.insertion_data.as_ptr(),
            modification_data: sparse_set.modification_data.as_ptr(),
            last_insertion: sparse_set.last_insert,
            last_modification: sparse_set.last_modification,
            current: sparse_set.current,
            is_tracking_modification,
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

impl<T: Component> Clone for FullRawWindow<'_, T, T::Tracking> {
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
            is_tracking_modification: self.is_tracking_modification,
            _phantom: PhantomData,
        }
    }
}

pub struct FullRawWindowMut<'a, T, Tracking> {
    sparse: *mut *mut EntityId,
    sparse_len: usize,
    pub(crate) dense: *mut EntityId,
    pub(crate) dense_len: usize,
    pub(crate) data: *mut T,
    pub(crate) insertion_data: *const u32,
    pub(crate) modification_data: *mut u32,
    pub(crate) last_insertion: u32,
    pub(crate) last_modification: u32,
    pub(crate) current: u32,
    pub(crate) is_tracking_modification: bool,
    _phantom: PhantomData<(&'a mut T, Tracking)>,
}

unsafe impl<T: Send + Component> Send for FullRawWindowMut<'_, T, T::Tracking> {}

impl<'w, T: Component> FullRawWindowMut<'w, T, T::Tracking> {
    #[inline]
    pub(crate) fn new(sparse_set: &mut ViewMut<'_, T, T::Tracking>) -> Self {
        let sparse_len = sparse_set.sparse.len();
        let sparse: *mut Option<Box<[EntityId; super::BUCKET_SIZE]>> =
            sparse_set.sparse.as_mut_ptr();
        let sparse = sparse as *mut *mut EntityId;
        let is_tracking_modification = sparse_set.is_tracking_modification();

        FullRawWindowMut {
            sparse,
            sparse_len,
            dense: sparse_set.dense.as_mut_ptr(),
            dense_len: sparse_set.dense.len(),
            data: sparse_set.data.as_mut_ptr(),
            insertion_data: sparse_set.insertion_data.as_ptr(),
            modification_data: sparse_set.modification_data.as_mut_ptr(),
            last_insertion: sparse_set.last_insert,
            last_modification: sparse_set.last_modification,
            current: sparse_set.current,
            is_tracking_modification,
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

impl<T: Component> Clone for FullRawWindowMut<'_, T, T::Tracking> {
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
