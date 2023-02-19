use crate::all_storages::AllStorages;
use crate::atomic_refcell::{ExclusiveBorrow, Ref, RefMut, SharedBorrow};
use crate::component::{Component, Unique};
use crate::entities::Entities;
use crate::entity_id::EntityId;
use crate::error;
use crate::get::Get;
use crate::sparse_set::{SparseSet, SparseSetDrain};
use crate::storage::StorageId;
use crate::track;
use crate::tracking::{
    is_track_within_bounds, DeletionTracking, Inserted, InsertedOrModified, InsertionTracking,
    ModificationTracking, Modified, RemovalOrDeletionTracking, RemovalTracking, Track, Tracking,
};
use crate::unique::UniqueStorage;
use core::fmt;
use core::ops::{Deref, DerefMut};

/// Shared view over `AllStorages`.
pub struct AllStoragesView<'a>(pub(crate) Ref<'a, &'a AllStorages>);

impl Clone for AllStoragesView<'_> {
    #[inline]
    fn clone(&self) -> Self {
        AllStoragesView(self.0.clone())
    }
}

impl Deref for AllStoragesView<'_> {
    type Target = AllStorages;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<AllStorages> for AllStoragesView<'_> {
    #[inline]
    fn as_ref(&self) -> &AllStorages {
        &self.0
    }
}

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
        self.entities
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
        self.entities
    }
}

impl DerefMut for EntitiesViewMut<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.entities
    }
}

/// Shared view over a component storage.
pub struct View<'a, T: Component, const TRACK: u32 = { track::Untracked }> {
    pub(crate) sparse_set: &'a SparseSet<T>,
    pub(crate) all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) borrow: Option<SharedBorrow<'a>>,
    pub(crate) last_insertion: u32,
    pub(crate) last_modification: u32,
    pub(crate) last_removal_or_deletion: u32,
    pub(crate) current: u32,
}

impl<'a, T: Component> View<'a, T, { track::Untracked }> {
    /// Creates a new [`View`] for custom [`SparseSet`] storage.
    ///
    /// ```
    /// use shipyard::{track, Component, SparseSet, StorageId, View, World};
    ///
    /// struct ScriptingComponent(Vec<u8>);
    /// impl Component for ScriptingComponent {}
    ///
    /// let world = World::new();
    ///
    /// world.add_custom_storage(
    ///     StorageId::Custom(0),
    ///     SparseSet::<ScriptingComponent>::new_custom_storage(),
    /// ).unwrap();
    ///
    /// let all_storages = world.all_storages().unwrap();
    /// let scripting_storage =
    ///     View::<ScriptingComponent>::new_for_custom_storage(StorageId::Custom(0), all_storages)
    ///         .unwrap();
    /// ```
    pub fn new_for_custom_storage(
        storage_id: StorageId,
        all_storages: Ref<'a, &'a AllStorages>,
    ) -> Result<Self, error::CustomStorageView> {
        use crate::all_storages::CustomStorageAccess;

        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };

        let storage = all_storages.custom_storage_by_id(storage_id)?;
        let (storage, borrow) = unsafe { Ref::destructure(storage) };

        if let Some(sparse_set) = storage.as_any().downcast_ref() {
            Ok(View {
                sparse_set,
                all_borrow: Some(all_borrow),
                borrow: Some(borrow),
                last_insertion: 0,
                last_modification: 0,
                last_removal_or_deletion: 0,
                current: 0,
            })
        } else {
            Err(error::CustomStorageView::WrongType(storage.name()))
        }
    }
}

impl<const TRACK: u32, T: Component> View<'_, T, TRACK>
where
    Track<TRACK>: InsertionTracking,
{
    /// Wraps this view to be able to iterate *inserted* components.
    #[inline]
    pub fn inserted(&self) -> Inserted<&Self> {
        Inserted(self)
    }

    /// Inside a workload returns `true` if `entity`'s component was inserted since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was inserted since the last call to [`clear_all_inserted`](ViewMut::clear_all_inserted).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_inserted(&self, entity: EntityId) -> bool {
        Track::<TRACK>::is_inserted(self.sparse_set, entity, self.last_insertion, self.current)
    }
}

impl<const TRACK: u32, T: Component> View<'_, T, TRACK>
where
    Track<TRACK>: ModificationTracking,
{
    /// Wraps this view to be able to iterate *modified* components.
    #[inline]
    pub fn modified(&self) -> Modified<&Self> {
        Modified(self)
    }

    /// Inside a workload returns `true` if `entity`'s component was modified since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was modified since the last call to [`clear_all_modified`](ViewMut::clear_all_modified).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_modified(&self, entity: EntityId) -> bool {
        Track::<TRACK>::is_modified(
            self.sparse_set,
            entity,
            self.last_modification,
            self.current,
        )
    }
}

impl<const TRACK: u32, T: Component> View<'_, T, TRACK>
where
    Track<TRACK>: InsertionTracking + ModificationTracking,
{
    /// Wraps this view to be able to iterate *inserted* and *modified* components.
    #[inline]
    pub fn inserted_or_modified(&self) -> InsertedOrModified<&Self> {
        InsertedOrModified(self)
    }

    /// Inside a workload returns `true` if `entity`'s component was inserted or modified since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was inserted or modified since the last call to [`clear_all_inserted`](ViewMut::clear_all_inserted).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_inserted_or_modified(&self, entity: EntityId) -> bool {
        Track::<TRACK>::is_inserted(self.sparse_set, entity, self.last_insertion, self.current)
            || Track::<TRACK>::is_modified(
                self.sparse_set,
                entity,
                self.last_modification,
                self.current,
            )
    }
}

impl<const TRACK: u32, T: Component> View<'_, T, TRACK>
where
    Track<TRACK>: DeletionTracking,
{
    /// Returns the *deleted* components of a storage tracking deletion.
    pub fn deleted(&self) -> impl Iterator<Item = (EntityId, &T)> + '_ {
        self.sparse_set
            .deletion_data
            .iter()
            .filter_map(move |(entity, timestamp, component)| {
                if is_track_within_bounds(*timestamp, self.last_removal_or_deletion, self.current) {
                    Some((*entity, component))
                } else {
                    None
                }
            })
    }

    /// Inside a workload returns `true` if `entity`'s component was deleted since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was deleted since the last call to [`clear_all_deleted`](SparseSet::clear_all_deleted).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_deleted(&self, entity: EntityId) -> bool {
        Track::<TRACK>::is_deleted(self, entity, self.last_removal_or_deletion, self.current)
    }
}

impl<const TRACK: u32, T: Component> View<'_, T, TRACK>
where
    Track<TRACK>: RemovalTracking,
{
    /// Returns the ids of *removed* components of a storage tracking removal.
    pub fn removed(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.sparse_set
            .removal_data
            .iter()
            .filter_map(move |(entity, timestamp)| {
                if is_track_within_bounds(*timestamp, self.last_removal_or_deletion, self.current) {
                    Some(*entity)
                } else {
                    None
                }
            })
    }

    /// Inside a workload returns `true` if `entity`'s component was removed since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was removed since the last call to [`clear_all_removed`](SparseSet::clear_all_removed).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_removed(&self, entity: EntityId) -> bool {
        Track::<TRACK>::is_removed(self, entity, self.last_removal_or_deletion, self.current)
    }
}

impl<const TRACK: u32, T: Component> View<'_, T, TRACK>
where
    Track<TRACK>: RemovalTracking + DeletionTracking,
{
    /// Returns the ids of *removed* or *deleted* components of a storage tracking removal and/or deletion.
    pub fn removed_or_deleted(&self) -> impl Iterator<Item = EntityId> + '_ {
        Track::<TRACK>::removed_or_deleted(self.sparse_set).filter_map(
            move |(entity, timestamp)| {
                if is_track_within_bounds(timestamp, self.last_removal_or_deletion, self.current) {
                    Some(entity)
                } else {
                    None
                }
            },
        )
    }

    /// Inside a workload returns `true` if `entity`'s component was deleted or removed since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was deleted or removed since the last clear call.\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_removed_or_deleted(&self, entity: EntityId) -> bool {
        Track::<TRACK>::is_deleted(self, entity, self.last_removal_or_deletion, self.current)
            || Track::<TRACK>::is_removed(self, entity, self.last_removal_or_deletion, self.current)
    }
}

impl<'a, T: Component, const TRACK: u32> Deref for View<'a, T, TRACK> {
    type Target = SparseSet<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.sparse_set
    }
}

impl<'a, T: Component, const TRACK: u32> AsRef<SparseSet<T>> for View<'a, T, TRACK> {
    #[inline]
    fn as_ref(&self) -> &SparseSet<T> {
        self.sparse_set
    }
}

impl<'a, T: Component, const TRACK: u32> Clone for View<'a, T, TRACK> {
    #[inline]
    fn clone(&self) -> Self {
        View {
            sparse_set: self.sparse_set,
            borrow: self.borrow.clone(),
            all_borrow: self.all_borrow.clone(),
            last_insertion: self.last_insertion,
            last_modification: self.last_modification,
            last_removal_or_deletion: self.last_removal_or_deletion,
            current: self.current,
        }
    }
}

impl<T: fmt::Debug + Component, const TRACK: u32> fmt::Debug for View<'_, T, TRACK> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.sparse_set.fmt(f)
    }
}

impl<T: Component, const TRACK: u32> core::ops::Index<EntityId> for View<'_, T, TRACK> {
    type Output = T;
    #[track_caller]
    #[inline]
    fn index(&self, entity: EntityId) -> &Self::Output {
        self.get(entity).unwrap()
    }
}

/// Exclusive view over a component storage.
pub struct ViewMut<'a, T: Component, const TRACK: u32 = { track::Untracked }> {
    pub(crate) sparse_set: &'a mut SparseSet<T>,
    pub(crate) _all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) _borrow: Option<ExclusiveBorrow<'a>>,
    pub(crate) last_insertion: u32,
    pub(crate) last_modification: u32,
    pub(crate) last_removal_or_deletion: u32,
    pub(crate) current: u32,
}

impl<'a, T: Component> ViewMut<'a, T, { track::Untracked }> {
    /// Creates a new [`ViewMut`] for custom [`SparseSet`] storage.
    ///
    /// ```
    /// use shipyard::{track, Component, SparseSet, StorageId, ViewMut, World};
    ///
    /// struct ScriptingComponent(Vec<u8>);
    /// impl Component for ScriptingComponent {}
    ///
    /// let world = World::new();
    ///
    /// world.add_custom_storage(
    ///     StorageId::Custom(0),
    ///     SparseSet::<ScriptingComponent>::new_custom_storage(),
    /// ).unwrap();
    ///
    /// let all_storages = world.all_storages().unwrap();
    /// let scripting_storage =
    ///     ViewMut::<ScriptingComponent>::new_for_custom_storage(StorageId::Custom(0), all_storages)
    ///         .unwrap();
    /// ```
    pub fn new_for_custom_storage(
        storage_id: StorageId,
        all_storages: Ref<'a, &'a AllStorages>,
    ) -> Result<Self, error::CustomStorageView> {
        use crate::all_storages::CustomStorageAccess;

        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };

        let storage = all_storages.custom_storage_mut_by_id(storage_id)?;
        let (storage, borrow) = unsafe { RefMut::destructure(storage) };

        let name = storage.name();

        if let Some(sparse_set) = storage.any_mut().downcast_mut() {
            Ok(ViewMut {
                sparse_set,
                _all_borrow: Some(all_borrow),
                _borrow: Some(borrow),
                last_insertion: 0,
                last_modification: 0,
                last_removal_or_deletion: 0,
                current: 0,
            })
        } else {
            Err(error::CustomStorageView::WrongType(name))
        }
    }
}

impl<'a, T: Component, const TRACK: u32> ViewMut<'a, T, TRACK>
where
    Track<TRACK>: Tracking,
{
    /// Deletes all components in this storage.
    pub fn clear(&mut self) {
        self.sparse_set.private_clear(self.current);
    }
    /// Creates a draining iterator that empties the storage and yields the removed items.
    pub fn drain(&mut self) -> SparseSetDrain<'_, T> {
        self.sparse_set.private_drain(self.current)
    }
    /// Applies the given function `f` to the entities `a` and `b`.\
    /// The two entities shouldn't point to the same component.  
    ///
    /// ### Panics
    ///
    /// - MissingComponent - if one of the entity doesn't have any component in the storage.
    /// - IdenticalIds - if the two entities point to the same component.
    #[track_caller]
    pub fn apply<R, F: FnOnce(&mut T, &T) -> R>(&mut self, a: EntityId, b: EntityId, f: F) -> R {
        self.sparse_set.private_apply(a, b, f, self.current)
    }
    /// Applies the given function `f` to the entities `a` and `b`.\
    /// The two entities shouldn't point to the same component.  
    ///
    /// ### Panics
    ///
    /// - MissingComponent - if one of the entity doesn't have any component in the storage.
    /// - IdenticalIds - if the two entities point to the same component.
    #[track_caller]
    pub fn apply_mut<R, F: FnOnce(&mut T, &mut T) -> R>(
        &mut self,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> R {
        self.sparse_set.private_apply_mut(a, b, f, self.current)
    }
}

impl<const TRACK: u32, T: Component> ViewMut<'_, T, TRACK>
where
    Track<TRACK>: InsertionTracking,
{
    /// Inside a workload returns `true` if `entity`'s component was inserted since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was inserted since the last call to [`clear_all_inserted`](ViewMut::clear_all_inserted).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_inserted(&self, entity: EntityId) -> bool {
        Track::<TRACK>::is_inserted(self.sparse_set, entity, self.last_insertion, self.current)
    }
    /// Wraps this view to be able to iterate *inserted* components.
    #[inline]
    pub fn inserted(&self) -> Inserted<&Self> {
        Inserted(self)
    }
    /// Wraps this view to be able to iterate *inserted* components.
    #[inline]
    pub fn inserted_mut(&mut self) -> Inserted<&mut Self> {
        Inserted(self)
    }
    /// Removes the *inserted* flag on all components of this storage.
    #[inline]
    pub fn clear_all_inserted(self) {
        self.sparse_set.private_clear_all_inserted(self.current);
    }
}

impl<const TRACK: u32, T: Component> ViewMut<'_, T, TRACK>
where
    Track<TRACK>: ModificationTracking,
{
    /// Inside a workload returns `true` if `entity`'s component was modified since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was modified since the last call to [`clear_all_modified`](ViewMut::clear_all_modified).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_modified(&self, entity: EntityId) -> bool {
        Track::<TRACK>::is_modified(
            self.sparse_set,
            entity,
            self.last_modification,
            self.current,
        )
    }
    /// Wraps this view to be able to iterate *modified* components.
    #[inline]
    pub fn modified(&self) -> Modified<&Self> {
        Modified(self)
    }
    /// Wraps this view to be able to iterate *modified* components.
    #[inline]
    pub fn modified_mut(&mut self) -> Modified<&mut Self> {
        Modified(self)
    }
    /// Removes the *modified* flag on all components of this storage.
    #[inline]
    pub fn clear_all_modified(self) {
        self.sparse_set.private_clear_all_modified(self.current);
    }
}

impl<const TRACK: u32, T: Component> ViewMut<'_, T, TRACK>
where
    Track<TRACK>: InsertionTracking + ModificationTracking,
{
    /// Inside a workload returns `true` if `entity`'s component was inserted or modified since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was inserted or modified since the last call to [`clear_all_inserted`](ViewMut::clear_all_inserted).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_inserted_or_modified(&self, entity: EntityId) -> bool {
        self.is_inserted(entity) || self.is_modified(entity)
    }
    /// Wraps this view to be able to iterate *inserted* and *modified* components.
    #[inline]
    pub fn inserted_or_modified(&self) -> InsertedOrModified<&Self> {
        InsertedOrModified(self)
    }
    /// Wraps this view to be able to iterate *inserted* and *modified* components.
    #[inline]
    pub fn inserted_or_modified_mut(&mut self) -> InsertedOrModified<&mut Self> {
        InsertedOrModified(self)
    }
    /// Removes the *inserted* and *modified* flags on all components of this storage.
    #[inline]
    pub fn clear_all_inserted_and_modified(self) {
        self.sparse_set
            .private_clear_all_inserted_and_modified(self.current);
    }
}

impl<const TRACK: u32, T: Component> ViewMut<'_, T, TRACK>
where
    Track<TRACK>: DeletionTracking,
{
    /// Inside a workload returns `true` if `entity`'s component was deleted since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was deleted since the last call to [`clear_all_deleted`](SparseSet::clear_all_deleted).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_deleted(&self, entity: EntityId) -> bool {
        Track::<TRACK>::is_deleted(self, entity, self.last_removal_or_deletion, self.current)
    }
    /// Returns the *deleted* components of a storage tracking deletion.
    pub fn deleted(&self) -> impl Iterator<Item = (EntityId, &T)> + '_ {
        self.sparse_set
            .deletion_data
            .iter()
            .filter_map(move |(entity, timestamp, component)| {
                if is_track_within_bounds(*timestamp, self.last_removal_or_deletion, self.current) {
                    Some((*entity, component))
                } else {
                    None
                }
            })
    }
}

impl<const TRACK: u32, T: Component> ViewMut<'_, T, TRACK>
where
    Track<TRACK>: RemovalTracking,
{
    /// Inside a workload returns `true` if `entity`'s component was removed since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was removed since the last call to [`clear_all_removed`](SparseSet::clear_all_removed).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_removed(&self, entity: EntityId) -> bool {
        Track::<TRACK>::is_removed(self, entity, self.last_removal_or_deletion, self.current)
    }
    /// Returns the ids of *removed* components of a storage tracking removal.
    pub fn removed(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.sparse_set
            .removal_data
            .iter()
            .filter_map(move |(entity, timestamp)| {
                if is_track_within_bounds(*timestamp, self.last_removal_or_deletion, self.current) {
                    Some(*entity)
                } else {
                    None
                }
            })
    }
}

impl<const TRACK: u32, T: Component> ViewMut<'_, T, TRACK>
where
    Track<TRACK>: RemovalOrDeletionTracking,
{
    /// Inside a workload returns `true` if `entity`'s component was deleted or removed since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was deleted or removed since the last clear call.\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_removed_or_deleted(&self, entity: EntityId) -> bool {
        Track::<TRACK>::is_removed(self, entity, self.last_removal_or_deletion, self.current)
            || Track::<TRACK>::is_deleted(self, entity, self.last_removal_or_deletion, self.current)
    }
    /// Returns the ids of *removed* or *deleted* components of a storage tracking removal and/or deletion.
    pub fn removed_or_deleted(&self) -> impl Iterator<Item = EntityId> + '_ {
        Track::removed_or_deleted(self.sparse_set).filter_map(move |(entity, timestamp)| {
            if is_track_within_bounds(timestamp, self.last_removal_or_deletion, self.current) {
                Some(entity)
            } else {
                None
            }
        })
    }
}

impl<T: Component, const TRACK: u32> Deref for ViewMut<'_, T, TRACK> {
    type Target = SparseSet<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.sparse_set
    }
}

impl<T: Component, const TRACK: u32> DerefMut for ViewMut<'_, T, TRACK> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.sparse_set
    }
}

impl<'a, T: Component, const TRACK: u32> AsRef<SparseSet<T>> for ViewMut<'a, T, TRACK> {
    #[inline]
    fn as_ref(&self) -> &SparseSet<T> {
        self.sparse_set
    }
}

impl<'a, T: Component, const TRACK: u32> AsMut<SparseSet<T>> for ViewMut<'a, T, TRACK> {
    #[inline]
    fn as_mut(&mut self) -> &mut SparseSet<T> {
        self.sparse_set
    }
}

impl<'a, T: Component, const TRACK: u32> AsMut<Self> for ViewMut<'a, T, TRACK> {
    #[inline]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T: fmt::Debug + Component, const TRACK: u32> fmt::Debug for ViewMut<'_, T, TRACK> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.sparse_set.fmt(f)
    }
}

impl<'a, T: Component, const TRACK: u32> core::ops::Index<EntityId> for ViewMut<'a, T, TRACK> {
    type Output = T;
    #[inline]
    fn index(&self, entity: EntityId) -> &Self::Output {
        self.get(entity).unwrap()
    }
}

impl<'a, T: Component, const TRACK: u32> core::ops::IndexMut<EntityId> for ViewMut<'a, T, TRACK> {
    #[inline]
    fn index_mut(&mut self, entity: EntityId) -> &mut Self::Output {
        let index = self
            .index_of(entity)
            .ok_or_else(|| error::MissingComponent {
                id: entity,
                name: core::any::type_name::<T>(),
            })
            .unwrap();

        let SparseSet {
            data,
            modification_data,
            is_tracking_modification,
            ..
        } = self.sparse_set;

        if *is_tracking_modification {
            unsafe {
                *modification_data.get_unchecked_mut(index) = self.current;
            };
        }

        unsafe { data.get_unchecked_mut(index) }
    }
}

/// Shared view over a unique component storage.
pub struct UniqueView<'a, T: Unique> {
    pub(crate) unique: &'a UniqueStorage<T>,
    pub(crate) borrow: Option<SharedBorrow<'a>>,
    pub(crate) all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) last_insertion: u32,
    pub(crate) last_modification: u32,
    pub(crate) current: u32,
}

impl<T: Unique> UniqueView<'_, T> {
    /// Duplicates the [`UniqueView`].
    #[allow(clippy::should_implement_trait)]
    #[inline]
    pub fn clone(unique: &Self) -> Self {
        UniqueView {
            unique: unique.unique,
            borrow: unique.borrow.clone(),
            all_borrow: unique.all_borrow.clone(),
            last_insertion: unique.last_insertion,
            last_modification: unique.last_modification,
            current: unique.current,
        }
    }
}

impl<T: Unique> UniqueView<'_, T> {
    /// Returns `true` if the component was inserted before the last [`clear_inserted`] call.  
    ///
    /// [`clear_inserted`]: UniqueViewMut::clear_inserted
    #[inline]
    pub fn is_inserted(&self) -> bool {
        is_track_within_bounds(self.unique.insert, self.last_insertion, self.current)
    }
    /// Returns `true` is the component was modified since the last [`clear_modified`] call.
    ///
    /// [`clear_modified`]: UniqueViewMut::clear_modified
    #[inline]
    pub fn is_modified(&self) -> bool {
        is_track_within_bounds(
            self.unique.modification,
            self.last_modification,
            self.current,
        )
    }
    /// Returns `true` if the component was inserted or modified since the last [`clear_inserted`] or [`clear_modified`] call.  
    ///
    /// [`clear_inserted`]: UniqueViewMut::clear_inserted
    /// [`clear_modified`]: UniqueViewMut::clear_modified
    #[inline]
    pub fn is_inserted_or_modified(&self) -> bool {
        self.is_inserted() || self.is_modified()
    }
}

impl<T: Unique> Deref for UniqueView<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.unique.value
    }
}

impl<T: Unique> AsRef<T> for UniqueView<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.unique.value
    }
}

impl<T: fmt::Debug + Unique> fmt::Debug for UniqueView<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.unique.value.fmt(f)
    }
}

/// Exclusive view over a unique component storage.
pub struct UniqueViewMut<'a, T: Unique> {
    pub(crate) unique: &'a mut UniqueStorage<T>,
    pub(crate) _borrow: Option<ExclusiveBorrow<'a>>,
    pub(crate) _all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) last_insertion: u32,
    pub(crate) last_modification: u32,
    pub(crate) current: u32,
}

impl<T: Unique> UniqueViewMut<'_, T> {
    /// Returns `true` if the component was inserted before the last [`clear_inserted`] call.  
    ///
    /// [`clear_inserted`]: Self::clear_inserted
    #[inline]
    pub fn is_inserted(&self) -> bool {
        is_track_within_bounds(self.unique.insert, self.last_insertion, self.current)
    }
    /// Returns `true` if the component was modified since the last [`clear_modified`] call.  
    ///
    /// [`clear_modified`]: Self::clear_modified
    #[inline]
    pub fn is_modified(&self) -> bool {
        is_track_within_bounds(
            self.unique.modification,
            self.last_modification,
            self.current,
        )
    }
    /// Returns `true` if the component was inserted or modified since the last [`clear_inserted`] or [`clear_modified`] call.  
    ///
    /// [`clear_inserted`]: Self::clear_inserted
    /// [`clear_modified`]: Self::clear_modified
    #[inline]
    pub fn is_inserted_or_modified(&self) -> bool {
        self.is_inserted() || self.is_modified()
    }
    /// Removes the *inserted* flag on the component of this storage.
    #[inline]
    pub fn clear_inserted(self) {
        self.unique.last_insert = self.current;
    }
    /// Removes the *modified* flag on the component of this storage.
    #[inline]
    pub fn clear_modified(self) {
        self.unique.last_modification = self.current;
    }
    /// Removes the *inserted* and *modified* flags on the component of this storage.
    #[inline]
    pub fn clear_inserted_and_modified(self) {
        self.unique.last_insert = self.current;
        self.unique.last_modification = self.current;
    }
}

impl<T: Unique> Deref for UniqueViewMut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.unique.value
    }
}

impl<T: Unique> DerefMut for UniqueViewMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.unique.modification = self.current;

        &mut self.unique.value
    }
}

impl<T: Unique> AsRef<T> for UniqueViewMut<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.unique.value
    }
}

impl<T: Unique> AsMut<T> for UniqueViewMut<'_, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self.unique.modification = self.current;

        &mut self.unique.value
    }
}

impl<T: fmt::Debug + Unique> fmt::Debug for UniqueViewMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.unique.value.fmt(f)
    }
}
