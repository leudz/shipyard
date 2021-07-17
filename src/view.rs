use crate::all_storages::AllStorages;
use crate::atomic_refcell::{ExclusiveBorrow, RefMut, SharedBorrow};
use crate::component::Component;
use crate::entities::Entities;
use crate::sparse_set::SparseSet;
use crate::track;
use crate::tracking::{Inserted, InsertedOrModified, Modified};
use crate::unique::Unique;
use core::fmt;
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
    pub(crate) entities: &'a Entities,
    pub(crate) borrow: Option<SharedBorrow<'a>>,
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
            entities: self.entities,
            borrow: self.borrow.clone(),
            all_borrow: self.all_borrow.clone(),
        }
    }
}

/// Exclusive view over `Entities` storage.
pub struct EntitiesViewMut<'a> {
    pub(crate) entities: &'a mut Entities,
    pub(crate) _borrow: Option<ExclusiveBorrow<'a>>,
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
        self.entities
    }
}

/// Shared view over a component storage.
pub struct View<'a, T: Component> {
    pub(crate) sparse_set: &'a SparseSet<T, T::Tracking>,
    pub(crate) borrow: Option<SharedBorrow<'a>>,
    pub(crate) all_borrow: Option<SharedBorrow<'a>>,
}

impl<T: Component> View<'_, T> {
    /// Wraps this view to be able to iterate *inserted* components.
    pub fn inserted(&self) -> Inserted<&Self> {
        Inserted(self)
    }
    /// Wraps this view to be able to iterate *modified* components.
    pub fn modified(&self) -> Modified<&Self> {
        Modified(self)
    }
    /// Wraps this view to be able to iterate *inserted* and *modified* components.
    pub fn inserted_or_modified(&self) -> InsertedOrModified<&Self> {
        InsertedOrModified(self)
    }
}

impl<'a, T: Component> Deref for View<'a, T> {
    type Target = SparseSet<T, T::Tracking>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.sparse_set
    }
}

impl<'a, T: Component> AsRef<SparseSet<T, T::Tracking>> for View<'a, T> {
    #[inline]
    fn as_ref(&self) -> &SparseSet<T, T::Tracking> {
        &self.sparse_set
    }
}

impl<'a, T: Component> Clone for View<'a, T> {
    #[inline]
    fn clone(&self) -> Self {
        View {
            sparse_set: self.sparse_set,
            borrow: self.borrow.clone(),
            all_borrow: self.all_borrow.clone(),
        }
    }
}

impl<T: fmt::Debug + Component> fmt::Debug for View<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.sparse_set.fmt(f)
    }
}

/// Exclusive view over a component storage.
pub struct ViewMut<'a, T: Component, Tracking: track::Tracking = <T as Component>::Tracking> {
    pub(crate) sparse_set: &'a mut SparseSet<T, Tracking>,
    pub(crate) _borrow: Option<ExclusiveBorrow<'a>>,
    pub(crate) _all_borrow: Option<SharedBorrow<'a>>,
}

impl<T: Component> ViewMut<'_, T, T::Tracking> {
    /// Wraps this view to be able to iterate *inserted* components.
    pub fn inserted(&self) -> Inserted<&Self> {
        Inserted(self)
    }
    /// Wraps this view to be able to iterate *modified* components.
    pub fn modified(&self) -> Modified<&Self> {
        Modified(self)
    }
    /// Wraps this view to be able to iterate *inserted* and *modified* components.
    pub fn inserted_or_modified(&self) -> InsertedOrModified<&Self> {
        InsertedOrModified(self)
    }
    /// Wraps this view to be able to iterate *inserted* components.
    pub fn inserted_mut(&mut self) -> Inserted<&mut Self> {
        Inserted(self)
    }
    /// Wraps this view to be able to iterate *modified* components.
    pub fn modified_mut(&mut self) -> Modified<&mut Self> {
        Modified(self)
    }
    /// Wraps this view to be able to iterate *inserted* and *modified* components.
    pub fn inserted_or_modified_mut(&mut self) -> InsertedOrModified<&mut Self> {
        InsertedOrModified(self)
    }
}

impl<T: Component> Deref for ViewMut<'_, T> {
    type Target = SparseSet<T, T::Tracking>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.sparse_set
    }
}

impl<T: Component> DerefMut for ViewMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.sparse_set
    }
}

impl<'a, T: Component> AsRef<SparseSet<T, T::Tracking>> for ViewMut<'a, T> {
    #[inline]
    fn as_ref(&self) -> &SparseSet<T, T::Tracking> {
        &self.sparse_set
    }
}

impl<'a, T: Component> AsMut<SparseSet<T, T::Tracking>> for ViewMut<'a, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut SparseSet<T, T::Tracking> {
        self.sparse_set
    }
}

impl<'a, T: Component> AsMut<Self> for ViewMut<'a, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T: fmt::Debug + Component> fmt::Debug for ViewMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.sparse_set.fmt(f)
    }
}

/// Shared view over a unique component storage.
pub struct UniqueView<'a, T: Component> {
    pub(crate) unique: &'a Unique<T>,
    pub(crate) borrow: Option<SharedBorrow<'a>>,
    pub(crate) all_borrow: Option<SharedBorrow<'a>>,
}

impl<T: Component> UniqueView<'_, T> {
    /// Returns `true` is the component was modified since the last [`clear_modified`] call.
    ///
    /// [`clear_modified`]: UniqueViewMut::clear_modified
    pub fn is_modified(unique: &Self) -> bool {
        unique.unique.is_modified
    }
}

impl<T: Component> Deref for UniqueView<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.unique.value
    }
}

impl<T: Component> AsRef<T> for UniqueView<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.unique.value
    }
}

impl<T: Component> Clone for UniqueView<'_, T> {
    #[inline]
    fn clone(&self) -> Self {
        UniqueView {
            unique: self.unique,
            borrow: self.borrow.clone(),
            all_borrow: self.all_borrow.clone(),
        }
    }
}

impl<T: fmt::Debug + Component> fmt::Debug for UniqueView<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.unique.value.fmt(f)
    }
}

/// Exclusive view over a unique component storage.
pub struct UniqueViewMut<'a, T: Component> {
    pub(crate) unique: &'a mut Unique<T>,
    pub(crate) _borrow: Option<ExclusiveBorrow<'a>>,
    pub(crate) _all_borrow: Option<SharedBorrow<'a>>,
}

impl<T: Component> UniqueViewMut<'_, T> {
    /// Returns `true` is the component was modified since the last [`clear_modified`] call.
    ///
    /// [`clear_modified`]: Self::clear_modified
    pub fn is_modified(unique: &Self) -> bool {
        unique.unique.is_modified
    }
    /// Removes the *modified* flag on this component.
    pub fn clear_modified(unique: &mut Self) {
        unique.unique.is_modified = false;
    }
}

impl<T: Component> Deref for UniqueViewMut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.unique.value
    }
}

impl<T: Component> DerefMut for UniqueViewMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.unique.is_modified = true;
        &mut self.unique.value
    }
}

impl<T: Component> AsRef<T> for UniqueViewMut<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.unique.value
    }
}

impl<T: Component> AsMut<T> for UniqueViewMut<'_, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self.unique.is_modified = true;
        &mut self.unique.value
    }
}

impl<T: fmt::Debug + Component> fmt::Debug for UniqueViewMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.unique.value.fmt(f)
    }
}
