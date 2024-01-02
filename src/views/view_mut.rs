use crate::all_storages::AllStorages;
use crate::atomic_refcell::{ARef, ARefMut, ExclusiveBorrow, SharedBorrow};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::get::Get;
use crate::r#mut::Mut;
use crate::sparse_set::{SparseSet, SparseSetDrain};
use crate::storage::StorageId;
use crate::track;
use crate::tracking::{
    DeletionTracking, Inserted, InsertedOrModified, InsertionTracking, ModificationTracking,
    Modified, RemovalOrDeletionTracking, RemovalTracking, Track, Tracking,
};
use crate::views::view::View;
use crate::{error, TrackingTimestamp};
use core::fmt;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

/// Exclusive view over a component storage.
pub struct ViewMut<'a, T: Component, TRACK = track::Untracked> {
    pub(crate) sparse_set: &'a mut SparseSet<T>,
    pub(crate) all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) borrow: ExclusiveBorrow<'a>,
    pub(crate) last_insertion: TrackingTimestamp,
    pub(crate) last_modification: TrackingTimestamp,
    pub(crate) last_removal_or_deletion: TrackingTimestamp,
    pub(crate) current: TrackingTimestamp,
    pub(crate) phantom: PhantomData<TRACK>,
}

impl<'a, T: Component, TRACK> ViewMut<'a, T, TRACK>
where
    Track<TRACK>: Tracking,
{
    /// Returns a `View` reborrowing from `ViewMut`.
    pub fn as_view(&self) -> View<'_, T, TRACK> {
        View {
            sparse_set: self.sparse_set,
            all_borrow: self.all_borrow.as_ref().cloned(),
            borrow: self.borrow.shared_reborrow(),
            last_insertion: self.last_insertion,
            last_modification: self.last_modification,
            last_removal_or_deletion: self.last_removal_or_deletion,
            current: self.current,
            phantom: PhantomData,
        }
    }

    /// Replaces the timestamp starting the tracking time window for insertions.
    ///
    /// Tracking works based on a time window. From the last time the system ran (in workloads)
    /// or since the last clear.
    ///
    /// Sometimes this automatic time window isn't what you need.
    /// This can happen when you want to keep the same tracking information for multiple runs
    /// of the same system.
    ///
    /// For example if you interpolate movement between frames, you might run an interpolation workload
    /// multiple times but not change the [`World`](crate::World) during its execution.\
    /// In this case you want the same tracking information for all runs of this workload
    /// which would have disappeared using the automatic window.
    pub fn override_last_insertion(
        &mut self,
        new_timestamp: TrackingTimestamp,
    ) -> TrackingTimestamp {
        core::mem::replace(&mut self.last_insertion, new_timestamp)
    }

    /// Replaces the timestamp starting the tracking time window for modifications.
    ///
    /// Tracking works based on a time window. From the last time the system ran (in workloads)
    /// or since the last clear.
    ///
    /// Sometimes this automatic time window isn't what you need.
    /// This can happen when you want to keep the same tracking information for multiple runs
    /// of the same system.
    ///
    /// For example if you interpolate movement between frames, you might run an interpolation workload
    /// multiple times but not change the [`World`](crate::World) during its execution.\
    /// In this case you want the same tracking information for all runs of this workload
    /// which would have disappeared using the automatic window.
    pub fn override_last_modification(
        &mut self,
        new_timestamp: TrackingTimestamp,
    ) -> TrackingTimestamp {
        core::mem::replace(&mut self.last_modification, new_timestamp)
    }

    /// Replaces the timestamp starting the tracking time window for removals and deletions.
    ///
    /// Tracking works based on a time window. From the last time the system ran (in workloads)
    /// or since the last clear.
    ///
    /// Sometimes this automatic time window isn't what you need.
    /// This can happen when you want to keep the same tracking information for multiple runs
    /// of the same system.
    ///
    /// For example if you interpolate movement between frames, you might run an interpolation workload
    /// multiple times but not change the [`World`](crate::World) during its execution.\
    /// In this case you want the same tracking information for all runs of this workload
    /// which would have disappeared using the automatic window.
    pub fn override_last_removal_or_deletion(
        &mut self,
        new_timestamp: TrackingTimestamp,
    ) -> TrackingTimestamp {
        core::mem::replace(&mut self.last_removal_or_deletion, new_timestamp)
    }
}

impl<'a, T: Component> ViewMut<'a, T, track::Untracked> {
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
        all_storages: ARef<'a, &'a AllStorages>,
    ) -> Result<Self, error::CustomStorageView> {
        use crate::all_storages::CustomStorageAccess;

        let (all_storages, all_borrow) = unsafe { ARef::destructure(all_storages) };

        let storage = all_storages.custom_storage_mut_by_id(storage_id)?;
        let (storage, borrow) = unsafe { ARefMut::destructure(storage) };

        let name = storage.name();

        if let Some(sparse_set) = storage.any_mut().downcast_mut() {
            Ok(ViewMut {
                sparse_set,
                all_borrow: Some(all_borrow),
                borrow,
                last_insertion: TrackingTimestamp::new(0),
                last_modification: TrackingTimestamp::new(0),
                last_removal_or_deletion: TrackingTimestamp::new(0),
                current: TrackingTimestamp::new(0),
                phantom: PhantomData,
            })
        } else {
            Err(error::CustomStorageView::WrongType(name))
        }
    }
}

impl<'a, T: Component, TRACK> ViewMut<'a, T, TRACK>
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

    /// Deletes all components for which `f(id, &component)` returns `false`.
    pub fn retain<F: FnMut(EntityId, &T) -> bool>(&mut self, f: F) {
        self.sparse_set.private_retain(self.current, f);
    }

    /// Deletes all components for which `f(id, Mut<component>)` returns `false`.
    pub fn retain_mut<F: FnMut(EntityId, Mut<'_, T>) -> bool>(&mut self, f: F) {
        self.sparse_set.private_retain_mut(self.current, f);
    }
}

impl<TRACK, T: Component + Default> ViewMut<'_, T, TRACK> {
    /// Retrieve `entity` component.
    ///
    /// If the entity doesn't have the component, insert its `Default` value.
    #[inline]
    pub fn get_or_default(&mut self, entity: EntityId) -> Mut<'_, T> {
        self.get_or_insert(entity, T::default())
    }
}

impl<TRACK, T: Component> ViewMut<'_, T, TRACK> {
    /// Retrieve `entity` component.
    ///
    /// If the entity doesn't have the component, insert `component`.
    #[inline]
    pub fn get_or_insert(&mut self, entity: EntityId, component: T) -> Mut<'_, T> {
        if !self.sparse_set.contains(entity) {
            self.sparse_set.insert(entity, component, self.current);
        }

        let index = self.index_of(entity).unwrap();

        let SparseSet {
            data,
            modification_data,
            is_tracking_modification,
            ..
        } = self.sparse_set;

        Mut {
            flag: is_tracking_modification
                .then(|| unsafe { modification_data.get_unchecked_mut(index) }),
            current: self.current,
            data: unsafe { data.get_unchecked_mut(index) },
        }
    }
    /// Retrieve `entity` component.
    ///
    /// If the entity doesn't have the component, insert the result of `f`.
    #[inline]
    pub fn get_or_insert_with<F: FnOnce() -> T>(&mut self, entity: EntityId, f: F) -> Mut<'_, T> {
        self.get_or_insert(entity, f())
    }
}

impl<TRACK, T: Component> ViewMut<'_, T, TRACK>
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

impl<TRACK, T: Component> ViewMut<'_, T, TRACK>
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

impl<TRACK, T: Component> ViewMut<'_, T, TRACK>
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

impl<TRACK, T: Component> ViewMut<'_, T, TRACK>
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
                if timestamp.is_within(self.last_removal_or_deletion, self.current) {
                    Some((*entity, component))
                } else {
                    None
                }
            })
    }
}

impl<TRACK, T: Component> ViewMut<'_, T, TRACK>
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
                if timestamp.is_within(self.last_removal_or_deletion, self.current) {
                    Some(*entity)
                } else {
                    None
                }
            })
    }
}

impl<TRACK, T: Component> ViewMut<'_, T, TRACK>
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
            if timestamp.is_within(self.last_removal_or_deletion, self.current) {
                Some(entity)
            } else {
                None
            }
        })
    }
}

impl<T: Component, TRACK> Deref for ViewMut<'_, T, TRACK> {
    type Target = SparseSet<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.sparse_set
    }
}

impl<T: Component, TRACK> DerefMut for ViewMut<'_, T, TRACK> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.sparse_set
    }
}

impl<'a, T: Component, TRACK> AsRef<SparseSet<T>> for ViewMut<'a, T, TRACK> {
    #[inline]
    fn as_ref(&self) -> &SparseSet<T> {
        self.sparse_set
    }
}

impl<'a, T: Component, TRACK> AsMut<SparseSet<T>> for ViewMut<'a, T, TRACK> {
    #[inline]
    fn as_mut(&mut self) -> &mut SparseSet<T> {
        self.sparse_set
    }
}

impl<'a, T: Component, TRACK> AsMut<Self> for ViewMut<'a, T, TRACK> {
    #[inline]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T: fmt::Debug + Component, TRACK> fmt::Debug for ViewMut<'_, T, TRACK> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.sparse_set.fmt(f)
    }
}

impl<'a, T: Component, TRACK> core::ops::Index<EntityId> for ViewMut<'a, T, TRACK> {
    type Output = T;
    #[inline]
    fn index(&self, entity: EntityId) -> &Self::Output {
        self.get(entity).unwrap()
    }
}

impl<'a, T: Component, TRACK> core::ops::IndexMut<EntityId> for ViewMut<'a, T, TRACK> {
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
