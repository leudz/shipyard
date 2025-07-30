use crate::all_storages::AllStorages;
use crate::borrow::{NonSend, NonSendSync, NonSync};
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::memory_usage::StorageMemoryUsage;
use crate::sparse_set::{sparse_array::SparseArray, SparseSet, BUCKET_SIZE};
use crate::storage::{SBoxBuilder, Storage, StorageId};
use crate::tracking::TrackingTimestamp;

impl<T: Component + Sync> Storage for NonSend<SparseSet<T>> {
    #[inline]
    fn delete(&mut self, entity: EntityId, current: TrackingTimestamp) {
        self.dyn_delete(entity, current);
    }
    #[inline]
    fn clear(&mut self, current: TrackingTimestamp) {
        self.private_clear(current);
    }
    fn sparse_array(&self) -> Option<&SparseArray<EntityId, BUCKET_SIZE>> {
        Some(&self.sparse)
    }
    fn memory_usage(&self) -> Option<StorageMemoryUsage> {
        Some(self.private_memory_usage())
    }
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    fn clear_all_removed_and_deleted(&mut self) {
        self.deletion_data.clear();
        self.removal_data.clear();
    }
    fn clear_all_removed_and_deleted_older_than_timestamp(&mut self, timestamp: TrackingTimestamp) {
        self.deletion_data
            .retain(|(_, t, _)| timestamp.is_older_than(*t));

        self.removal_data
            .retain(|(_, t)| timestamp.is_older_than(*t));
    }
    #[inline]
    #[track_caller]
    fn move_component_from(
        &mut self,
        other_all_storages: &mut AllStorages,
        from: EntityId,
        to: EntityId,
        current: TrackingTimestamp,
        other_current: TrackingTimestamp,
    ) {
        if let Some(component) = self.dyn_remove(from, current) {
            let other_sparse_set = other_all_storages.exclusive_storage_or_insert_non_send_mut(
                StorageId::of::<NonSend<SparseSet<T>>>(),
                || NonSend(SparseSet::<T>::new()),
            );

            let _ = other_sparse_set.insert(to, component, other_current);
        }
    }
    fn try_clone(&self, other_current: TrackingTimestamp) -> Option<SBoxBuilder> {
        self.clone.map(|clone| {
            let mut sparse_set = SparseSet::<T>::new();

            sparse_set.is_tracking_insertion = self.is_tracking_insertion;
            sparse_set.is_tracking_modification = self.is_tracking_modification;
            sparse_set.is_tracking_deletion = self.is_tracking_deletion;
            sparse_set.is_tracking_removal = self.is_tracking_removal;

            sparse_set.sparse = self.sparse.clone();
            sparse_set.dense = self.dense.clone();
            sparse_set.data = self.data.iter().map(clone).collect();

            if sparse_set.is_tracking_insertion {
                sparse_set
                    .insertion_data
                    .resize(self.dense.len(), other_current);
            }
            if sparse_set.is_tracking_modification {
                sparse_set
                    .modification_data
                    .resize(self.dense.len(), TrackingTimestamp::origin());
            }

            SBoxBuilder::new(NonSend(sparse_set))
        })
    }

    fn clone_component_to(
        &self,
        other_all_storages: &mut AllStorages,
        from: EntityId,
        to: EntityId,
        other_current: TrackingTimestamp,
    ) {
        if let Some(clone) = &self.clone {
            if let Some(component) = self.private_get(from) {
                let other_sparse_set = other_all_storages.exclusive_storage_or_insert_non_send_mut(
                    StorageId::of::<NonSend<SparseSet<T>>>(),
                    || NonSend(SparseSet::<T>::new()),
                );

                let _ = other_sparse_set.insert(to, (clone)(component), other_current);
            }
        }
    }
}

impl<T: Component + Send> Storage for NonSync<SparseSet<T>> {
    #[inline]
    fn delete(&mut self, entity: EntityId, current: TrackingTimestamp) {
        self.dyn_delete(entity, current);
    }
    #[inline]
    fn clear(&mut self, current: TrackingTimestamp) {
        self.private_clear(current);
    }
    fn sparse_array(&self) -> Option<&SparseArray<EntityId, BUCKET_SIZE>> {
        Some(&self.sparse)
    }
    fn memory_usage(&self) -> Option<StorageMemoryUsage> {
        Some(self.private_memory_usage())
    }
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    fn clear_all_removed_and_deleted(&mut self) {
        self.deletion_data.clear();
        self.removal_data.clear();
    }
    fn clear_all_removed_and_deleted_older_than_timestamp(&mut self, timestamp: TrackingTimestamp) {
        self.deletion_data
            .retain(|(_, t, _)| timestamp.is_older_than(*t));

        self.removal_data
            .retain(|(_, t)| timestamp.is_older_than(*t));
    }
    #[inline]
    fn move_component_from(
        &mut self,
        other_all_storages: &mut AllStorages,
        from: EntityId,
        to: EntityId,
        current: TrackingTimestamp,
        other_current: TrackingTimestamp,
    ) {
        if let Some(component) = self.dyn_remove(from, current) {
            let other_sparse_set = other_all_storages.exclusive_storage_or_insert_non_sync_mut(
                StorageId::of::<NonSync<SparseSet<T>>>(),
                || NonSync(SparseSet::<T>::new()),
            );

            let _ = other_sparse_set.insert(to, component, other_current);
        }
    }
    fn try_clone(&self, other_current: TrackingTimestamp) -> Option<SBoxBuilder> {
        self.clone.map(|clone| {
            let mut sparse_set = SparseSet::<T>::new();

            sparse_set.is_tracking_insertion = self.is_tracking_insertion;
            sparse_set.is_tracking_modification = self.is_tracking_modification;
            sparse_set.is_tracking_deletion = self.is_tracking_deletion;
            sparse_set.is_tracking_removal = self.is_tracking_removal;

            sparse_set.sparse = self.sparse.clone();
            sparse_set.dense = self.dense.clone();
            sparse_set.data = self.data.iter().map(clone).collect();

            if sparse_set.is_tracking_insertion {
                sparse_set
                    .insertion_data
                    .resize(self.dense.len(), other_current);
            }
            if sparse_set.is_tracking_modification {
                sparse_set
                    .modification_data
                    .resize(self.dense.len(), TrackingTimestamp::origin());
            }

            SBoxBuilder::new(NonSync(sparse_set))
        })
    }

    fn clone_component_to(
        &self,
        other_all_storages: &mut AllStorages,
        from: EntityId,
        to: EntityId,
        other_current: TrackingTimestamp,
    ) {
        if let Some(clone) = &self.clone {
            if let Some(component) = self.private_get(from) {
                let other_sparse_set = other_all_storages.exclusive_storage_or_insert_non_sync_mut(
                    StorageId::of::<NonSync<SparseSet<T>>>(),
                    || NonSync(SparseSet::<T>::new()),
                );

                let _ = other_sparse_set.insert(to, (clone)(component), other_current);
            }
        }
    }
}

impl<T: Component> Storage for NonSendSync<SparseSet<T>> {
    #[inline]
    fn delete(&mut self, entity: EntityId, current: TrackingTimestamp) {
        self.dyn_delete(entity, current);
    }
    #[inline]
    fn clear(&mut self, current: TrackingTimestamp) {
        self.private_clear(current);
    }
    fn sparse_array(&self) -> Option<&SparseArray<EntityId, BUCKET_SIZE>> {
        Some(&self.sparse)
    }
    fn memory_usage(&self) -> Option<StorageMemoryUsage> {
        Some(self.private_memory_usage())
    }
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    fn clear_all_removed_and_deleted(&mut self) {
        self.deletion_data.clear();
        self.removal_data.clear();
    }
    fn clear_all_removed_and_deleted_older_than_timestamp(&mut self, timestamp: TrackingTimestamp) {
        self.deletion_data
            .retain(|(_, t, _)| timestamp.is_older_than(*t));

        self.removal_data
            .retain(|(_, t)| timestamp.is_older_than(*t));
    }
    #[inline]
    fn move_component_from(
        &mut self,
        other_all_storages: &mut AllStorages,
        from: EntityId,
        to: EntityId,
        current: TrackingTimestamp,
        other_current: TrackingTimestamp,
    ) {
        if let Some(component) = self.dyn_remove(from, current) {
            let other_sparse_set = other_all_storages
                .exclusive_storage_or_insert_non_send_sync_mut(
                    StorageId::of::<NonSendSync<SparseSet<T>>>(),
                    || NonSendSync(SparseSet::<T>::new()),
                );

            let _ = other_sparse_set.insert(to, component, other_current);
        }
    }

    fn try_clone(&self, other_current: TrackingTimestamp) -> Option<SBoxBuilder> {
        self.clone.map(|clone| {
            let mut sparse_set = SparseSet::<T>::new();

            sparse_set.is_tracking_insertion = self.is_tracking_insertion;
            sparse_set.is_tracking_modification = self.is_tracking_modification;
            sparse_set.is_tracking_deletion = self.is_tracking_deletion;
            sparse_set.is_tracking_removal = self.is_tracking_removal;

            sparse_set.sparse = self.sparse.clone();
            sparse_set.dense = self.dense.clone();
            sparse_set.data = self.data.iter().map(clone).collect();

            if sparse_set.is_tracking_insertion {
                sparse_set
                    .insertion_data
                    .resize(self.dense.len(), other_current);
            }
            if sparse_set.is_tracking_modification {
                sparse_set
                    .modification_data
                    .resize(self.dense.len(), TrackingTimestamp::origin());
            }

            SBoxBuilder::new(NonSendSync(sparse_set))
        })
    }

    fn clone_component_to(
        &self,
        other_all_storages: &mut AllStorages,
        from: EntityId,
        to: EntityId,
        other_current: TrackingTimestamp,
    ) {
        if let Some(clone) = &self.clone {
            if let Some(component) = self.private_get(from) {
                let other_sparse_set = other_all_storages
                    .exclusive_storage_or_insert_non_send_sync_mut(
                        StorageId::of::<NonSendSync<SparseSet<T>>>(),
                        || NonSendSync(SparseSet::<T>::new()),
                    );

                let _ = other_sparse_set.insert(to, (clone)(component), other_current);
            }
        }
    }
}
