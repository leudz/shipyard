use crate::all_storages::AllStorages;
use crate::atomic_refcell::{ExclusiveBorrow, RefMut, SharedBorrow};
use crate::component::Component;
use crate::entities::Entities;
use crate::sparse_set::SparseSet;
use crate::track::{self, Tracking};
use crate::tracking::{Inserted, InsertedOrModified, Modified};
use crate::unique::{TrackingState, Unique};
use core::fmt;
use core::marker::PhantomData;
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
pub struct View<'a, T: Component, Tracking: track::Tracking = <T as Component>::Tracking> {
    pub(crate) sparse_set: &'a SparseSet<T, Tracking>,
    pub(crate) borrow: Option<SharedBorrow<'a>>,
    pub(crate) all_borrow: Option<SharedBorrow<'a>>,
}

impl<T: Component> View<'_, T, track::Insertion> {
    /// Wraps this view to be able to iterate *inserted* components.
    pub fn inserted(&self) -> Inserted<&Self> {
        Inserted(self)
    }
    /// Wraps this view to be able to iterate *inserted* and *modified* components.
    pub fn inserted_or_modified(&self) -> InsertedOrModified<&Self> {
        InsertedOrModified(self)
    }
}

impl<T: Component> View<'_, T, track::Modification> {
    /// Wraps this view to be able to iterate *modified* components.
    pub fn modified(&self) -> Modified<&Self> {
        Modified(self)
    }
    /// Wraps this view to be able to iterate *inserted* and *modified* components.
    pub fn inserted_or_modified(&self) -> InsertedOrModified<&Self> {
        InsertedOrModified(self)
    }
}

impl<T: Component> View<'_, T, track::All> {
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

impl<T: Component> ViewMut<'_, T, track::Insertion> {
    /// Wraps this view to be able to iterate *inserted* components.
    pub fn inserted(&self) -> Inserted<&Self> {
        Inserted(self)
    }
    /// Wraps this view to be able to iterate *inserted* and *modified* components.
    pub fn inserted_or_modified(&self) -> InsertedOrModified<&Self> {
        InsertedOrModified(self)
    }
    /// Wraps this view to be able to iterate *inserted* components.
    pub fn inserted_mut(&mut self) -> Inserted<&mut Self> {
        Inserted(self)
    }
    /// Wraps this view to be able to iterate *inserted* and *modified* components.
    pub fn inserted_or_modified_mut(&mut self) -> InsertedOrModified<&mut Self> {
        InsertedOrModified(self)
    }
}

impl<T: Component> ViewMut<'_, T, track::Modification> {
    /// Wraps this view to be able to iterate *modified* components.
    pub fn modified(&self) -> Modified<&Self> {
        Modified(self)
    }
    /// Wraps this view to be able to iterate *inserted* and *modified* components.
    pub fn inserted_or_modified(&self) -> InsertedOrModified<&Self> {
        InsertedOrModified(self)
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

impl<T: Component> ViewMut<'_, T, track::All> {
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
pub struct UniqueView<'a, T: Component, Track: Tracking = <T as Component>::Tracking> {
    pub(crate) unique: &'a Unique<T>,
    pub(crate) borrow: Option<SharedBorrow<'a>>,
    pub(crate) all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) _phantom: PhantomData<Track>,
}

impl<T: Component<Tracking = track::Insertion>> UniqueView<'_, T, track::Insertion> {
    /// Returns `true` if the component was inserted before the last [`clear_inserted`] call.  
    ///
    /// [`clear_inserted`]: UniqueViewMut::clear_inserted
    pub fn is_inserted(&self) -> bool {
        self.unique.tracking == TrackingState::Inserted
    }
    /// Returns `true` if the component was inserted before the last [`clear_inserted`] call.  
    ///
    /// [`clear_inserted`]: UniqueViewMut::clear_inserted
    pub fn is_inserted_or_modified(&self) -> bool {
        self.unique.tracking == TrackingState::Inserted
    }
}

impl<T: Component<Tracking = track::Modification>> UniqueView<'_, T, track::Modification> {
    /// Returns `true` is the component was modified since the last [`clear_modified`] call.
    ///
    /// [`clear_modified`]: UniqueViewMut::clear_modified
    pub fn is_modified(&self) -> bool {
        self.unique.tracking == TrackingState::Modified
    }
    /// Returns `true` if the component was modified since the last [`clear_modified`] call.  
    ///
    /// [`clear_modified`]: UniqueViewMut::clear_modified
    pub fn is_inserted_or_modified(&self) -> bool {
        self.unique.tracking == TrackingState::Modified
    }
}

impl<T: Component<Tracking = track::All>> UniqueView<'_, T, track::All> {
    /// Returns `true` if the component was inserted before the last [`clear_inserted`] call.  
    ///
    /// [`clear_inserted`]: UniqueViewMut::clear_inserted
    pub fn is_inserted(&self) -> bool {
        self.unique.tracking == TrackingState::Inserted
    }
    /// Returns `true` is the component was modified since the last [`clear_modified`] call.
    ///
    /// [`clear_modified`]: UniqueViewMut::clear_modified
    pub fn is_modified(&self) -> bool {
        self.unique.tracking == TrackingState::Modified
    }
    /// Returns `true` if the component was inserted or modified since the last [`clear_inserted`] or [`clear_modified`] call.  
    ///
    /// [`clear_inserted`]: UniqueViewMut::clear_inserted
    /// [`clear_modified`]: UniqueViewMut::clear_modified
    pub fn is_inserted_or_modified(&self) -> bool {
        self.unique.tracking != TrackingState::Nothing
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
            _phantom: PhantomData,
        }
    }
}

impl<T: fmt::Debug + Component> fmt::Debug for UniqueView<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.unique.value.fmt(f)
    }
}

/// Exclusive view over a unique component storage.
pub struct UniqueViewMut<'a, T: Component, Track: Tracking = <T as Component>::Tracking> {
    pub(crate) unique: &'a mut Unique<T>,
    pub(crate) _borrow: Option<ExclusiveBorrow<'a>>,
    pub(crate) _all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) _phantom: PhantomData<Track>,
}

impl<T: Component<Tracking = track::Insertion>> UniqueViewMut<'_, T, track::Insertion> {
    /// Returns `true` if the component was inserted before the last [`clear_inserted`] call.  
    ///
    /// [`clear_inserted`]: Self::clear_inserted
    pub fn is_inserted(&self) -> bool {
        self.unique.tracking == TrackingState::Inserted
    }
    /// Returns `true` if the component was inserted before the last [`clear_inserted`] call.  
    ///
    /// [`clear_inserted`]: Self::clear_inserted
    pub fn is_inserted_or_modified(&self) -> bool {
        self.unique.tracking == TrackingState::Inserted
    }
    /// Removes the *inserted* flag on the component of this storage.
    pub fn clear_inserted(&mut self) {
        self.unique.tracking = TrackingState::Nothing;
    }
}

impl<T: Component<Tracking = track::Modification>> UniqueViewMut<'_, T, track::Modification> {
    /// Returns `true` if the component was modified since the last [`clear_modified`] call.  
    ///
    /// [`clear_modified`]: Self::clear_modified
    pub fn is_modified(&self) -> bool {
        self.unique.tracking == TrackingState::Modified
    }
    /// Returns `true` if the component was modified since the last [`clear_modified`] call.  
    ///
    /// [`clear_modified`]: Self::clear_modified
    pub fn is_inserted_or_modified(&self) -> bool {
        self.unique.tracking == TrackingState::Modified
    }
    /// Removes the *medified* flag on the component of this storage.
    pub fn clear_modified(&mut self) {
        self.unique.tracking = TrackingState::Nothing;
    }
}

impl<T: Component<Tracking = track::All>> UniqueViewMut<'_, T, track::All> {
    /// Returns `true` if the component was inserted before the last [`clear_inserted`] call.  
    ///
    /// [`clear_inserted`]: Self::clear_inserted
    pub fn is_inserted(&self) -> bool {
        self.unique.tracking == TrackingState::Inserted
    }
    /// Returns `true` if the component was modified since the last [`clear_modified`] call.  
    ///
    /// [`clear_modified`]: Self::clear_modified
    pub fn is_modified(&self) -> bool {
        self.unique.tracking == TrackingState::Modified
    }
    /// Returns `true` if the component was inserted or modified since the last [`clear_inserted`] or [`clear_modified`] call.  
    ///
    /// [`clear_inserted`]: Self::clear_inserted
    /// [`clear_modified`]: Self::clear_modified
    pub fn is_inserted_or_modified(&self) -> bool {
        self.unique.tracking != TrackingState::Nothing
    }
    /// Removes the *inserted* flag on the component of this storage.
    pub fn clear_inserted(&mut self) {
        if self.unique.tracking == TrackingState::Inserted {
            self.unique.tracking = TrackingState::Nothing;
        }
    }
    /// Removes the *medified* flag on the component of this storage.
    pub fn clear_modified(&mut self) {
        if self.unique.tracking == TrackingState::Modified {
            self.unique.tracking = TrackingState::Nothing;
        }
    }
    /// Removes the *inserted* and *modified* flags on the component of this storage.
    pub fn clear_inserted_and_modified(&mut self) {
        self.unique.tracking = TrackingState::Nothing;
    }
}

impl<T: Component> Deref for UniqueViewMut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.unique.value
    }
}

impl<T: Component<Tracking = track::Nothing>> DerefMut for UniqueViewMut<'_, T, track::Nothing> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.unique.value
    }
}

impl<T: Component<Tracking = track::Insertion>> DerefMut
    for UniqueViewMut<'_, T, track::Insertion>
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.unique.value
    }
}

impl<T: Component<Tracking = track::Removal>> DerefMut for UniqueViewMut<'_, T, track::Removal> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.unique.value
    }
}

impl<T: Component<Tracking = track::Modification>> DerefMut
    for UniqueViewMut<'_, T, track::Modification>
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.unique.tracking = TrackingState::Modified;

        &mut self.unique.value
    }
}

impl<T: Component<Tracking = track::All>> DerefMut for UniqueViewMut<'_, T, track::All> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        if self.unique.tracking == TrackingState::Nothing {
            self.unique.tracking = TrackingState::Modified;
        }

        &mut self.unique.value
    }
}

impl<T: Component> AsRef<T> for UniqueViewMut<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.unique.value
    }
}

impl<T: Component<Tracking = track::Nothing>> AsMut<T> for UniqueViewMut<'_, T, track::Nothing> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        &mut self.unique.value
    }
}

impl<T: Component<Tracking = track::Insertion>> AsMut<T>
    for UniqueViewMut<'_, T, track::Insertion>
{
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        &mut self.unique.value
    }
}

impl<T: Component<Tracking = track::Removal>> AsMut<T> for UniqueViewMut<'_, T, track::Removal> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        &mut self.unique.value
    }
}

impl<T: Component<Tracking = track::Modification>> AsMut<T>
    for UniqueViewMut<'_, T, track::Modification>
{
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self.unique.tracking = TrackingState::Modified;

        &mut self.unique.value
    }
}

impl<T: Component<Tracking = track::All>> AsMut<T> for UniqueViewMut<'_, T, track::All> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        if self.unique.tracking == TrackingState::Nothing {
            self.unique.tracking = TrackingState::Modified;
        }

        &mut self.unique.value
    }
}

impl<T: fmt::Debug + Component> fmt::Debug for UniqueViewMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.unique.value.fmt(f)
    }
}
