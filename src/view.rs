use crate::atomic_refcell::{Ref, RefMut, SharedBorrow};
use crate::pack::update::{Inserted, InsertedOrModified, Modified};
use crate::sparse_set::SparseSet;
use crate::storage::{AllStorages, Entities, Unique};
use core::ops::{Deref, DerefMut};

/// Exclusive view over `AllStorages`.
pub struct AllStoragesViewMut<'a>(pub(crate) RefMut<'a, &'a mut AllStorages>);

impl Deref for AllStoragesViewMut<'_> {
    type Target = AllStorages;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AllStoragesViewMut<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<AllStorages> for AllStoragesViewMut<'_> {
    #[inline]
    fn as_ref(&self) -> &AllStorages {
        &self.0
    }
}

impl AsMut<AllStorages> for AllStoragesViewMut<'_> {
    #[inline]
    fn as_mut(&mut self) -> &mut AllStorages {
        &mut self.0
    }
}

/// Shared view over `Entities` storage.
pub struct EntitiesView<'a> {
    pub(crate) entities: Ref<'a, &'a Entities>,
    pub(crate) all_borrow: Option<SharedBorrow<'a>>,
}

impl Deref for EntitiesView<'_> {
    type Target = Entities;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.entities
    }
}

impl Clone for EntitiesView<'_> {
    #[inline]
    fn clone(&self) -> Self {
        EntitiesView {
            entities: self.entities.clone(),
            all_borrow: self.all_borrow.clone(),
        }
    }
}

/// Exclusive view over `Entities` storage.
pub struct EntitiesViewMut<'a> {
    pub(crate) entities: RefMut<'a, &'a mut Entities>,
    pub(crate) _all_borrow: Option<SharedBorrow<'a>>,
}

impl Deref for EntitiesViewMut<'_> {
    type Target = Entities;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.entities
    }
}

impl DerefMut for EntitiesViewMut<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.entities
    }
}

/// Shared view over a component storage.
pub struct View<'a, T> {
    pub(crate) sparse_set: Ref<'a, &'a SparseSet<T>>,
    pub(crate) all_borrow: Option<SharedBorrow<'a>>,
}

impl<T> View<'_, T> {
    pub fn inserted(&self) -> Inserted<&Self> {
        Inserted(self)
    }
    pub fn modified(&self) -> Modified<&Self> {
        Modified(self)
    }
    pub fn inserted_or_modified(&self) -> InsertedOrModified<&Self> {
        InsertedOrModified(self)
    }
}

impl<'a, T> Deref for View<'a, T> {
    type Target = SparseSet<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.sparse_set
    }
}

impl<'a, T> AsRef<SparseSet<T>> for View<'a, T> {
    #[inline]
    fn as_ref(&self) -> &SparseSet<T> {
        &self.sparse_set
    }
}

impl<'a, T> Clone for View<'a, T> {
    #[inline]
    fn clone(&self) -> Self {
        View {
            sparse_set: self.sparse_set.clone(),
            all_borrow: self.all_borrow.clone(),
        }
    }
}

/// Exclusive view over a component storage.
pub struct ViewMut<'a, T> {
    pub(crate) sparse_set: RefMut<'a, &'a mut SparseSet<T>>,
    pub(crate) _all_borrow: Option<SharedBorrow<'a>>,
}

impl<T> ViewMut<'_, T> {
    pub fn inserted(&self) -> Inserted<&Self> {
        Inserted(self)
    }
    pub fn modified(&self) -> Modified<&Self> {
        Modified(self)
    }
    pub fn inserted_or_modified(&self) -> InsertedOrModified<&Self> {
        InsertedOrModified(self)
    }
    pub fn inserted_mut(&mut self) -> Inserted<&mut Self> {
        Inserted(self)
    }
    pub fn modified_mut(&mut self) -> Modified<&mut Self> {
        Modified(self)
    }
    pub fn inserted_or_modified_mut(&mut self) -> InsertedOrModified<&mut Self> {
        InsertedOrModified(self)
    }
}

impl<T> Deref for ViewMut<'_, T> {
    type Target = SparseSet<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.sparse_set
    }
}

impl<T> DerefMut for ViewMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sparse_set
    }
}

impl<'a, T> AsRef<SparseSet<T>> for ViewMut<'a, T> {
    #[inline]
    fn as_ref(&self) -> &SparseSet<T> {
        &self.sparse_set
    }
}

impl<'a, T> AsMut<SparseSet<T>> for ViewMut<'a, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut SparseSet<T> {
        &mut self.sparse_set
    }
}

/// Shared view over a unique component storage.
pub struct UniqueView<'a, T> {
    pub(crate) unique: Ref<'a, &'a Unique<T>>,
    pub(crate) all_borrow: Option<SharedBorrow<'a>>,
}

impl<T> Deref for UniqueView<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.unique.0
    }
}

impl<T> AsRef<T> for UniqueView<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.unique.0
    }
}

impl<T> Clone for UniqueView<'_, T> {
    #[inline]
    fn clone(&self) -> Self {
        UniqueView {
            unique: self.unique.clone(),
            all_borrow: self.all_borrow.clone(),
        }
    }
}

/// Exclusive view over a unique component storage.
pub struct UniqueViewMut<'a, T> {
    pub(crate) unique: RefMut<'a, &'a mut Unique<T>>,
    pub(crate) _all_borrow: Option<SharedBorrow<'a>>,
}

impl<T> Deref for UniqueViewMut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.unique.0
    }
}

impl<T> DerefMut for UniqueViewMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.unique.0
    }
}

impl<T> AsRef<T> for UniqueViewMut<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.unique.0
    }
}

impl<T> AsMut<T> for UniqueViewMut<'_, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        &mut self.unique.0
    }
}
