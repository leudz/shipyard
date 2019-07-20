use crate::atomic_refcell::{Borrow, Ref, RefMut};
use crate::component_storage::ComponentStorage;
use crate::error;
use crate::sparse_array::SparseArray;
use std::convert::TryFrom;

/// `Read` is a `Ref` with two levels of borrow.
/// First it borrows `AllComponent`,
/// then it borrows the storage itself.
pub struct Read<'a, T> {
    pub(crate) inner: &'a SparseArray<T>,
    pub(crate) _inner_borrow: Borrow<'a>,
    pub(crate) _outer_borrow: Borrow<'a>,
}

impl<T> std::ops::Deref for Read<'_, T> {
    type Target = SparseArray<T>;
    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl<T> std::convert::AsRef<SparseArray<T>> for Read<'_, T> {
    fn as_ref(&self) -> &SparseArray<T> {
        self.inner
    }
}

impl<'a, T: 'static> TryFrom<Ref<'a, ComponentStorage>> for Read<'a, T> {
    type Error = error::Borrow;
    fn try_from(storage: Ref<'a, ComponentStorage>) -> Result<Self, Self::Error> {
        // SAFE the order is respected
        // storage will disappear at the end of this function
        // while outer_borrow continue to live inside `Read`
        let (storage, outer_borrow) = unsafe { Ref::destructure(storage) };
        // SAFE the order is respected
        // array gets dropped first before the two borrows
        // inner_borrow gets dropped before outer_borrow
        let (array, inner_borrow) = unsafe { Ref::destructure(storage.array()?) };
        Ok(Read {
            inner: array,
            _inner_borrow: inner_borrow,
            _outer_borrow: outer_borrow,
        })
    }
}

/// `Write` is a `RefMut` with two levels of borrow.
/// First it borrows `AllComponent`,
/// then it borrows the storage itself.
pub struct Write<'a, T> {
    pub(crate) inner: &'a mut SparseArray<T>,
    pub(crate) _inner_borrow: Borrow<'a>,
    pub(crate) _outer_borrow: Borrow<'a>,
}

impl<T> std::ops::Deref for Write<'_, T> {
    type Target = SparseArray<T>;
    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl<T> std::ops::DerefMut for Write<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
    }
}

impl<T> std::convert::AsRef<SparseArray<T>> for Write<'_, T> {
    fn as_ref(&self) -> &SparseArray<T> {
        self.inner
    }
}

impl<T> std::convert::AsMut<SparseArray<T>> for Write<'_, T> {
    fn as_mut(&mut self) -> &mut SparseArray<T> {
        self.inner
    }
}

impl<'a, T: 'static> TryFrom<Ref<'a, ComponentStorage>> for Write<'a, T> {
    type Error = error::Borrow;
    fn try_from(storage: Ref<'a, ComponentStorage>) -> Result<Self, Self::Error> {
        // SAFE the order is respected
        // storage will disappear at the end of this function
        // while outer_borrow continue to live inside `Write`
        let (storage, outer_borrow) = unsafe { Ref::destructure(storage) };
        // SAFE the order is respected
        // array gets dropped first before the two borrows
        // inner_borrow gets dropped before outer_borrow
        let (array, inner_borrow) = unsafe { RefMut::destructure(storage.array_mut()?) };
        Ok(Write {
            inner: array,
            _inner_borrow: inner_borrow,
            _outer_borrow: outer_borrow,
        })
    }
}
