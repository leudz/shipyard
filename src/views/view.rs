use crate::all_storages::AllStorages;
use crate::atomic_refcell::{ARef, SharedBorrow};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::error;
use crate::get::Get;
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
use crate::track;
use crate::tracking::{
    DeletionTracking, Inserted, InsertedOrModified, InsertionTracking, ModificationTracking,
    Modified, RemovalTracking, Tracking, TrackingTimestamp,
};
use core::fmt;
use core::marker::PhantomData;
use core::ops::Deref;

/// Shared view over a component storage.
pub struct View<'a, T: Component, Track: Tracking = <T as Component>::Tracking> {
    pub(crate) sparse_set: &'a SparseSet<T>,
    pub(crate) all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) borrow: SharedBorrow<'a>,
    pub(crate) last_insertion: TrackingTimestamp,
    pub(crate) last_modification: TrackingTimestamp,
    pub(crate) last_removal_or_deletion: TrackingTimestamp,
    pub(crate) current: TrackingTimestamp,
    pub(crate) phantom: PhantomData<Track>,
}

impl<'a, T: Component, Track: Tracking> View<'a, T, Track> {
    /// Check that the view tracks insertion if the component is tracking it.
    pub const ASSERT_VIEW_TRACKING_INSERTION: () = assert!(
        Track::VALUE & 0b0001 >= <T::Tracking as Tracking>::VALUE & 0b0001,
        "Component is tracking insertion but View isn't"
    );
    /// Check that the view tracks modification if the component is tracking it.
    pub const ASSERT_VIEW_TRACKING_MODIFICATION: () = assert!(
        Track::VALUE & 0b0010 >= <T::Tracking as Tracking>::VALUE & 0b0010,
        "Component is tracking modification but View isn't"
    );
    /// Check that the view tracks deletion if the component is tracking it.
    pub const ASSERT_VIEW_TRACKING_DELETION: () = assert!(
        Track::VALUE & 0b0100 >= <T::Tracking as Tracking>::VALUE & 0b0100,
        "Component is tracking deletion but View isn't"
    );
    /// Check that the view tracks removal if the component is tracking it.
    pub const ASSERT_VIEW_TRACKING_REMOVAL: () = assert!(
        Track::VALUE & 0b1000 >= <T::Tracking as Tracking>::VALUE & 0b1000,
        "Component is tracking removal but View isn't"
    );

    pub(crate) fn new(
        sparse_set: &'a SparseSet<T>,
        borrow: SharedBorrow<'a>,
        all_borrow: Option<SharedBorrow<'a>>,
        last_run: Option<TrackingTimestamp>,
        current: TrackingTimestamp,
    ) -> Self {
        let _: () = Self::ASSERT_VIEW_TRACKING_INSERTION;
        let _: () = Self::ASSERT_VIEW_TRACKING_MODIFICATION;
        let _: () = Self::ASSERT_VIEW_TRACKING_DELETION;
        let _: () = Self::ASSERT_VIEW_TRACKING_REMOVAL;

        Self {
            last_insertion: last_run.unwrap_or(sparse_set.last_insert),
            last_modification: last_run.unwrap_or(sparse_set.last_modified),
            last_removal_or_deletion: last_run.unwrap_or(TrackingTimestamp::origin()),
            current,
            sparse_set,
            borrow,
            all_borrow,
            phantom: PhantomData,
        }
    }
}

impl<'a, T: Component, Track> View<'a, T, Track>
where
    Track: Tracking,
{
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

impl<'a, T: Component> View<'a, T, track::Untracked> {
    /// Creates a new [`View`] for custom [`SparseSet`] storage.
    ///
    /// ```
    /// use shipyard::{track, Component, sparse_set::SparseSet, StorageId, View, World};
    ///
    /// struct ScriptingComponent(Vec<u8>);
    /// impl Component for ScriptingComponent {
    ///     type Tracking = track::Untracked;
    /// }
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
        all_storages: ARef<'a, &'a AllStorages>,
    ) -> Result<Self, error::CustomStorageView> {
        use crate::all_storages::CustomStorageAccess;

        let (all_storages, all_borrow) = unsafe { ARef::destructure(all_storages) };

        let storage = all_storages.custom_storage_by_id(storage_id)?;
        let (storage, borrow) = unsafe { ARef::destructure(storage) };

        if let Some(sparse_set) = storage.as_any().downcast_ref() {
            let _: () = Self::ASSERT_VIEW_TRACKING_INSERTION;
            let _: () = Self::ASSERT_VIEW_TRACKING_MODIFICATION;
            let _: () = Self::ASSERT_VIEW_TRACKING_DELETION;
            let _: () = Self::ASSERT_VIEW_TRACKING_REMOVAL;

            Ok(View {
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
            Err(error::CustomStorageView::WrongType(storage.name()))
        }
    }
}

impl<Track, T: Component> View<'_, T, Track>
where
    Track: InsertionTracking,
{
    /// Wraps this view to be able to iterate *inserted* components.
    #[inline]
    pub fn inserted(&self) -> Inserted<&Self> {
        Inserted(self)
    }

    /// Inside a workload returns `true` if `entity`'s component was inserted since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was inserted since the last call to [`clear_all_inserted`](crate::ViewMut::clear_all_inserted).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_inserted(&self, entity: EntityId) -> bool {
        Track::is_inserted(self.sparse_set, entity, self.last_insertion, self.current)
    }
}

impl<Track, T: Component> View<'_, T, Track>
where
    Track: ModificationTracking,
{
    /// Wraps this view to be able to iterate *modified* components.
    #[inline]
    pub fn modified(&self) -> Modified<&Self> {
        Modified(self)
    }

    /// Inside a workload returns `true` if `entity`'s component was modified since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was modified since the last call to [`clear_all_modified`](crate::ViewMut::clear_all_modified).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_modified(&self, entity: EntityId) -> bool {
        Track::is_modified(
            self.sparse_set,
            entity,
            self.last_modification,
            self.current,
        )
    }
}

impl<Track, T: Component> View<'_, T, Track>
where
    Track: InsertionTracking + ModificationTracking,
{
    /// Wraps this view to be able to iterate *inserted* and *modified* components.
    #[inline]
    pub fn inserted_or_modified(&self) -> InsertedOrModified<&Self> {
        InsertedOrModified(self)
    }

    /// Inside a workload returns `true` if `entity`'s component was inserted or modified since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was inserted or modified since the last call to [`clear_all_inserted`](crate::ViewMut::clear_all_inserted).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_inserted_or_modified(&self, entity: EntityId) -> bool {
        Track::is_inserted(self.sparse_set, entity, self.last_insertion, self.current)
            || Track::is_modified(
                self.sparse_set,
                entity,
                self.last_modification,
                self.current,
            )
    }
}

impl<Track, T: Component> View<'_, T, Track>
where
    Track: DeletionTracking,
{
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

    /// Inside a workload returns `true` if `entity`'s component was deleted since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was deleted since the last call to [`clear_all_deleted`](SparseSet::clear_all_deleted).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_deleted(&self, entity: EntityId) -> bool {
        Track::is_deleted(self, entity, self.last_removal_or_deletion, self.current)
    }
}

impl<Track, T: Component> View<'_, T, Track>
where
    Track: RemovalTracking,
{
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

    /// Inside a workload returns `true` if `entity`'s component was removed since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was removed since the last call to [`clear_all_removed`](SparseSet::clear_all_removed).\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_removed(&self, entity: EntityId) -> bool {
        Track::is_removed(self, entity, self.last_removal_or_deletion, self.current)
    }
}

impl<Track, T: Component> View<'_, T, Track>
where
    Track: RemovalTracking + DeletionTracking,
{
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

    /// Inside a workload returns `true` if `entity`'s component was deleted or removed since the last run of this system.\
    /// Outside workloads returns `true` if `entity`'s component was deleted or removed since the last clear call.\
    /// Returns `false` if `entity` does not have a component in this storage.
    #[inline]
    pub fn is_removed_or_deleted(&self, entity: EntityId) -> bool {
        Track::is_deleted(self, entity, self.last_removal_or_deletion, self.current)
            || Track::is_removed(self, entity, self.last_removal_or_deletion, self.current)
    }
}

impl<'a, T: Component, Track: Tracking> Deref for View<'a, T, Track> {
    type Target = SparseSet<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.sparse_set
    }
}

impl<'a, T: Component, Track: Tracking> AsRef<SparseSet<T>> for View<'a, T, Track> {
    #[inline]
    fn as_ref(&self) -> &SparseSet<T> {
        self.sparse_set
    }
}

impl<'a, T: Component, Track: Tracking> Clone for View<'a, T, Track> {
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
            phantom: PhantomData,
        }
    }
}

impl<T: fmt::Debug + Component, Track: Tracking> fmt::Debug for View<'_, T, Track> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.sparse_set.fmt(f)
    }
}

impl<T: Component, Track: Tracking> core::ops::Index<EntityId> for View<'_, T, Track> {
    type Output = T;
    #[track_caller]
    #[inline]
    fn index(&self, entity: EntityId) -> &Self::Output {
        self.get(entity).unwrap()
    }
}
