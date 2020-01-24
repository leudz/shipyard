use crate::sparse_set::SparseSet;
use crate::storage::Entities;
use crate::storage::EntityId;
use core::any::TypeId;

pub(super) trait UnknownStorage {
    fn delete(&mut self, entity: EntityId, storage_to_unpack: &mut Vec<TypeId>);
    fn unpack(&mut self, entitiy: EntityId);
    fn type_id(&self) -> TypeId
    where
        Self: 'static,
    {
        TypeId::of::<Self>()
    }
}

impl<T: 'static> UnknownStorage for SparseSet<T> {
    fn delete(&mut self, entity: EntityId, storage_to_unpack: &mut Vec<TypeId>) {
        self.actual_delete(entity);

        storage_to_unpack.reserve(self.pack_info.observer_types.len());

        let mut i = 0;
        for observer in self.pack_info.observer_types.iter().copied() {
            while i < storage_to_unpack.len() && observer < storage_to_unpack[i] {
                i += 1;
            }
            if storage_to_unpack.is_empty() || observer != storage_to_unpack[i] {
                storage_to_unpack.insert(i, observer);
            }
        }
    }
    fn unpack(&mut self, entity: EntityId) {
        Self::unpack(self, entity);
    }
}

impl dyn UnknownStorage {
    pub(crate) fn is<T: 'static>(&self) -> bool {
        TypeId::of::<T>() == self.type_id()
    }
    pub(crate) fn sparse_set<T: 'static>(&self) -> Option<&SparseSet<T>> {
        if self.is::<SparseSet<T>>() {
            // SAFE type matches
            unsafe {
                let ptr: *const _ = self;
                let ptr: *const SparseSet<T> = ptr as _;
                Some(&*ptr)
            }
        } else {
            None
        }
    }
    pub(crate) fn sparse_set_mut<T: 'static>(&mut self) -> Option<&mut SparseSet<T>> {
        if self.is::<SparseSet<T>>() {
            // SAFE type matches
            unsafe {
                let ptr: *mut _ = self;
                let ptr: *mut SparseSet<T> = ptr as _;
                Some(&mut *ptr)
            }
        } else {
            None
        }
    }
    pub(crate) fn entities(&self) -> Option<&Entities> {
        if self.is::<Entities>() {
            // SAFE type matches
            unsafe {
                let ptr: *const _ = self;
                let ptr: *const Entities = ptr as _;
                Some(&*ptr)
            }
        } else {
            None
        }
    }
    pub(crate) fn entities_mut(&mut self) -> Option<&mut Entities> {
        if self.is::<Entities>() {
            // SAFE type matches
            unsafe {
                let ptr: *mut _ = self;
                let ptr: *mut Entities = ptr as _;
                Some(&mut *ptr)
            }
        } else {
            None
        }
    }
}
