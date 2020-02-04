mod pack_info;
pub mod sort;
mod view_add_entity;
mod windows;

use crate::error;
use crate::storage::EntityId;
use crate::unknown_storage::UnknownStorage;
use core::any::{Any, TypeId};
use core::marker::PhantomData;
use core::ptr;
pub(crate) use pack_info::{LoosePack, Pack, PackInfo, TightPack, UpdatePack};
pub(crate) use view_add_entity::ViewAddEntity;
pub(crate) use windows::RawWindowMut;
pub use windows::{Window, WindowMut};

// A sparse array is a data structure with 2 vectors: one sparse, the other dense.
// Only usize can be added. On insertion, the number is pushed into the dense vector
// and sparse[number] is set to dense.len() - 1.
// For all number present in the sparse array, dense[sparse[number]] == number.
// For all other values if set sparse[number] will have any value left there
// and if set dense[sparse[number]] != number.
// We can't be limited to store solely integers, this is why there is a third vector.
// It mimics the dense vector in regard to insertion/deletion.
pub struct SparseSet<T> {
    pub(crate) sparse: Vec<usize>,
    pub(crate) dense: Vec<EntityId>,
    pub(crate) data: Vec<T>,
    pub(crate) pack_info: PackInfo<T>,
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        SparseSet {
            sparse: Vec::new(),
            dense: Vec::new(),
            data: Vec::new(),
            pack_info: Default::default(),
        }
    }
}

impl<T> SparseSet<T> {
    pub(crate) fn window(&self) -> Window<'_, T> {
        Window {
            sparse: &self.sparse,
            dense: &self.dense,
            data: &self.data,
            pack_info: &self.pack_info,
        }
    }
    pub(crate) fn window_mut(&mut self) -> WindowMut<'_, T> {
        WindowMut {
            sparse: &mut self.sparse,
            dense: &mut self.dense,
            data: &mut self.data,
            pack_info: &mut self.pack_info,
        }
    }
    pub(crate) fn raw_window_mut(&mut self) -> RawWindowMut<'_, T> {
        RawWindowMut {
            sparse: self.sparse.as_mut_ptr(),
            sparse_len: self.sparse.len(),
            dense: self.dense.as_mut_ptr(),
            dense_len: self.dense.len(),
            data: self.data.as_mut_ptr(),
            pack_info: &mut self.pack_info,
            _phantom: PhantomData,
        }
    }
    pub(crate) fn insert(&mut self, mut value: T, entity: EntityId) -> Option<T> {
        if entity.index() >= self.sparse.len() {
            self.sparse.resize(entity.index() + 1, 0);
        }

        let result = if let Some(data) = self.get_mut(entity) {
            std::mem::swap(data, &mut value);
            Some(value)
        } else {
            unsafe { *self.sparse.get_unchecked_mut(entity.index()) = self.dense.len() };
            self.dense.push(entity);
            self.data.push(value);
            None
        };

        if let Pack::Update(pack) = &mut self.pack_info.pack {
            let len = self.data.len() - 1;
            unsafe {
                *self.sparse.get_unchecked_mut(entity.index()) = pack.inserted;
                *self.sparse.get_unchecked_mut(
                    self.dense
                        .get_unchecked(pack.inserted + pack.modified)
                        .index(),
                ) = len;
                *self
                    .sparse
                    .get_unchecked_mut(self.dense.get_unchecked(pack.inserted).index()) =
                    pack.inserted + pack.modified;
            }
            self.dense.swap(pack.inserted + pack.modified, len);
            self.dense
                .swap(pack.inserted, pack.inserted + pack.modified);
            self.data.swap(pack.inserted + pack.modified, len);
            self.data.swap(pack.inserted, pack.inserted + pack.modified);
            pack.inserted += 1;
        }

        result
    }
    pub fn try_remove(&mut self, entity: EntityId) -> Result<Option<T>, error::Remove>
    where
        T: 'static,
    {
        if self.pack_info.observer_types.is_empty() {
            match self.pack_info.pack {
                Pack::Tight(_) => Err(error::Remove::MissingPackStorage(TypeId::of::<T>())),
                Pack::Loose(_) => Err(error::Remove::MissingPackStorage(TypeId::of::<T>())),
                _ => Ok(self.actual_remove(entity)),
            }
        } else {
            Err(error::Remove::MissingPackStorage(TypeId::of::<T>()))
        }
    }
    pub fn remove(&mut self, entity: EntityId) -> Option<T>
    where
        T: 'static,
    {
        self.try_remove(entity).unwrap()
    }
    pub(crate) fn actual_remove(&mut self, entity: EntityId) -> Option<T> {
        if entity.index() < self.sparse.len() {
            // SAFE we're inbound
            let mut dense_index = unsafe { *self.sparse.get_unchecked(entity.index()) };
            if dense_index < self.dense.len() {
                // SAFE we're inbound
                let dense_id = unsafe { *self.dense.get_unchecked(dense_index) };
                if dense_id.index() == entity.index() && dense_id.version() <= entity.version() {
                    match &mut self.pack_info.pack {
                        Pack::Tight(pack_info) => {
                            let pack_len = pack_info.len;
                            if dense_index < pack_len {
                                pack_info.len -= 1;
                                // swap index and last packed element (can be the same)
                                unsafe {
                                    *self.sparse.get_unchecked_mut(
                                        self.dense.get_unchecked(pack_len - 1).index(),
                                    ) = dense_index;
                                }
                                self.dense.swap(dense_index, pack_len - 1);
                                self.data.swap(dense_index, pack_len - 1);
                                dense_index = pack_len - 1;
                            }
                        }
                        Pack::Loose(pack_info) => {
                            let pack_len = pack_info.len;
                            if dense_index < pack_len {
                                pack_info.len -= 1;
                                // swap index and last packed element (can be the same)
                                unsafe {
                                    *self.sparse.get_unchecked_mut(
                                        self.dense.get_unchecked(pack_len - 1).index(),
                                    ) = dense_index;
                                }
                                self.dense.swap(dense_index, pack_len - 1);
                                self.data.swap(dense_index, pack_len - 1);
                                dense_index = pack_len - 1;
                            }
                        }
                        Pack::Update(pack) => {
                            if dense_index < pack.inserted {
                                pack.inserted -= 1;
                                unsafe {
                                    *self.sparse.get_unchecked_mut(
                                        self.dense.get_unchecked(pack.inserted).index(),
                                    ) = dense_index;
                                }
                                self.dense.swap(dense_index, pack.inserted);
                                self.data.swap(dense_index, pack.inserted);
                                dense_index = pack.inserted;
                            }
                            if dense_index < pack.inserted + pack.modified {
                                pack.modified -= 1;
                                unsafe {
                                    *self.sparse.get_unchecked_mut(
                                        self.dense
                                            .get_unchecked(pack.inserted + pack.modified)
                                            .index(),
                                    ) = dense_index;
                                }
                                self.dense.swap(dense_index, pack.inserted + pack.modified);
                                self.data.swap(dense_index, pack.inserted + pack.modified);
                                dense_index = pack.inserted + pack.modified;
                            }
                        }
                        Pack::NoPack => {}
                    }
                    unsafe {
                        *self.sparse.get_unchecked_mut(
                            self.dense.get_unchecked(self.dense.len() - 1).index(),
                        ) = dense_index;
                    }
                    self.dense.swap_remove(dense_index);
                    if dense_id.version() == entity.version() {
                        Some(self.data.swap_remove(dense_index))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }
    pub fn try_delete(&mut self, entity: EntityId) -> Result<(), error::Remove>
    where
        T: 'static,
    {
        if self.pack_info.observer_types.is_empty() {
            match self.pack_info.pack {
                Pack::Tight(_) => Err(error::Remove::MissingPackStorage(TypeId::of::<T>())),
                Pack::Loose(_) => Err(error::Remove::MissingPackStorage(TypeId::of::<T>())),
                _ => {
                    self.actual_delete(entity);
                    Ok(())
                }
            }
        } else {
            Err(error::Remove::MissingPackStorage(TypeId::of::<T>()))
        }
    }
    pub fn delete(&mut self, entity: EntityId)
    where
        T: 'static,
    {
        self.try_delete(entity).unwrap()
    }
    pub fn actual_delete(&mut self, entity: EntityId) {
        if let Some(component) = self.actual_remove(entity) {
            if let Pack::Update(pack) = &mut self.pack_info.pack {
                pack.deleted.push((entity, component));
            }
        }
    }
    pub fn contains(&self, entity: EntityId) -> bool {
        self.window().contains(entity)
    }
    pub(crate) fn get(&self, entity: EntityId) -> Option<&T> {
        if self.contains(entity) {
            Some(unsafe {
                self.data
                    .get_unchecked(*self.sparse.get_unchecked(entity.index()))
            })
        } else {
            None
        }
    }
    pub(crate) fn get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        if self.contains(entity) {
            // SAFE we checked the window countains the entity
            let mut index = unsafe { *self.sparse.get_unchecked(entity.index()) };
            if let Pack::Update(pack) = &mut self.pack_info.pack {
                if index >= pack.modified {
                    // index of the first element non modified
                    let non_mod = pack.inserted + pack.modified;
                    if index >= non_mod {
                        // SAFE we checked the window contains the entity
                        unsafe {
                            ptr::swap(
                                self.dense.get_unchecked_mut(non_mod),
                                self.dense.get_unchecked_mut(index),
                            );
                            ptr::swap(
                                self.data.get_unchecked_mut(non_mod),
                                self.data.get_unchecked_mut(index),
                            );
                            *self
                                .sparse
                                .get_unchecked_mut((*self.dense.get_unchecked(non_mod)).index()) =
                                non_mod;
                            *self
                                .sparse
                                .get_unchecked_mut((*self.dense.get_unchecked(index)).index()) =
                                index;
                        }
                        pack.modified += 1;
                        index = non_mod;
                    }
                }
            }
            Some(unsafe { self.data.get_unchecked_mut(index) })
        } else {
            None
        }
    }
    pub fn len(&self) -> usize {
        self.window().len()
    }
    pub fn is_empty(&self) -> bool {
        self.window().is_empty()
    }
    pub fn try_inserted(&self) -> Result<Window<'_, T>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(Window {
                sparse: &self.sparse,
                dense: &self.dense[0..pack.inserted],
                data: &self.data[0..pack.inserted],
                pack_info: &self.pack_info,
            })
        } else {
            Err(error::NotUpdatePack)
        }
    }
    pub fn inserted(&self) -> Window<'_, T> {
        self.try_inserted().unwrap()
    }
    pub fn try_inserted_mut(&mut self) -> Result<WindowMut<'_, T>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(WindowMut {
                sparse: &mut self.sparse,
                dense: &mut self.dense[0..pack.inserted],
                data: &mut self.data[0..pack.inserted],
                pack_info: &mut self.pack_info,
            })
        } else {
            Err(error::NotUpdatePack)
        }
    }
    pub fn inserted_mut(&mut self) -> WindowMut<'_, T> {
        self.try_inserted_mut().unwrap()
    }
    pub fn try_modified(&self) -> Result<Window<'_, T>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(Window {
                sparse: &self.sparse,
                dense: &self.dense[pack.inserted..pack.inserted + pack.modified],
                data: &self.data[pack.inserted..pack.inserted + pack.modified],
                pack_info: &self.pack_info,
            })
        } else {
            Err(error::NotUpdatePack)
        }
    }
    pub fn modified(&self) -> Window<'_, T> {
        self.try_modified().unwrap()
    }
    pub fn try_modified_mut(&mut self) -> Result<WindowMut<'_, T>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(WindowMut {
                sparse: &mut self.sparse,
                dense: &mut self.dense[pack.inserted..pack.inserted + pack.modified],
                data: &mut self.data[pack.inserted..pack.inserted + pack.modified],
                pack_info: &mut self.pack_info,
            })
        } else {
            Err(error::NotUpdatePack)
        }
    }
    pub fn modified_mut(&mut self) -> WindowMut<'_, T> {
        self.try_modified_mut().unwrap()
    }
    pub fn try_inserted_or_modified(&self) -> Result<Window<'_, T>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(Window {
                sparse: &self.sparse,
                dense: &self.dense[0..pack.inserted + pack.modified],
                data: &self.data[0..pack.inserted + pack.modified],
                pack_info: &self.pack_info,
            })
        } else {
            Err(error::NotUpdatePack)
        }
    }
    pub fn inserted_or_modified(&self) -> Window<'_, T> {
        self.try_inserted_or_modified().unwrap()
    }
    pub fn try_inserted_or_modified_mut(
        &mut self,
    ) -> Result<WindowMut<'_, T>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(WindowMut {
                sparse: &mut self.sparse,
                dense: &mut self.dense[0..pack.inserted + pack.modified],
                data: &mut self.data[0..pack.inserted + pack.modified],
                pack_info: &mut self.pack_info,
            })
        } else {
            Err(error::NotUpdatePack)
        }
    }
    pub fn inserted_or_modified_mut(&mut self) -> WindowMut<'_, T> {
        self.try_inserted_or_modified_mut().unwrap()
    }
    pub fn try_deleted(&self) -> Result<&[(EntityId, T)], error::NotUpdatePack> {
        if let Pack::Update(pack) = &self.pack_info.pack {
            Ok(&pack.deleted)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    pub fn deleted(&self) -> &[(EntityId, T)] {
        self.try_deleted().unwrap()
    }
    pub fn try_take_deleted(&mut self) -> Result<Vec<(EntityId, T)>, error::NotUpdatePack> {
        if let Pack::Update(pack) = &mut self.pack_info.pack {
            let mut vec = Vec::with_capacity(pack.deleted.capacity());
            std::mem::swap(&mut vec, &mut pack.deleted);
            Ok(vec)
        } else {
            Err(error::NotUpdatePack)
        }
    }
    pub fn take_deleted(&mut self) -> Vec<(EntityId, T)> {
        self.try_take_deleted().unwrap()
    }
    pub fn try_clear_inserted(&mut self) -> Result<(), error::NotUpdatePack> {
        if let Pack::Update(pack) = &mut self.pack_info.pack {
            if pack.modified == 0 {
                pack.inserted = 0;
            } else {
                let new_len = pack.inserted;
                while pack.inserted > 0 {
                    let new_end =
                        std::cmp::min(pack.inserted + pack.modified - 1, self.dense.len());
                    self.dense.swap(new_end, pack.inserted - 1);
                    self.data.swap(new_end, pack.inserted - 1);
                    pack.inserted -= 1;
                }
                for i in pack.modified.saturating_sub(new_len)..pack.modified + new_len {
                    unsafe {
                        *self
                            .sparse
                            .get_unchecked_mut(self.dense.get_unchecked(i).index()) = i;
                    }
                }
            }
            Ok(())
        } else {
            Err(error::NotUpdatePack)
        }
    }
    pub fn clear_inserted(&mut self) {
        self.try_clear_inserted().unwrap()
    }
    pub fn try_clear_modified(&mut self) -> Result<(), error::NotUpdatePack> {
        if let Pack::Update(pack) = &mut self.pack_info.pack {
            pack.modified = 0;
            Ok(())
        } else {
            Err(error::NotUpdatePack)
        }
    }
    pub fn clear_modified(&mut self) {
        self.try_clear_modified().unwrap()
    }
    pub fn try_clear_inserted_and_modified(&mut self) -> Result<(), error::NotUpdatePack> {
        if let Pack::Update(pack) = &mut self.pack_info.pack {
            pack.inserted = 0;
            pack.modified = 0;
            Ok(())
        } else {
            Err(error::NotUpdatePack)
        }
    }
    pub fn clear_inserted_and_modified(&mut self) {
        self.try_clear_inserted_and_modified().unwrap()
    }
    pub(crate) fn is_unique(&self) -> bool {
        self.window().is_unique()
    }
    //          ▼ old end of pack
    //              ▼ new end of pack
    // [_ _ _ _ | _ | _ _ _ _ _]
    //            ▲       ▼
    //            ---------
    //              pack
    pub(crate) fn pack(&mut self, entity: EntityId) {
        self.window_mut().pack(entity)
    }
    pub(crate) fn unpack(&mut self, entity: EntityId) {
        self.window_mut().unpack(entity)
    }
    /// Place the unique component in the storage.
    /// The storage has to be completely empty.
    pub(crate) fn insert_unique(&mut self, component: T) {
        if self.sparse.is_empty() && self.dense.is_empty() && self.data.is_empty() {
            self.data.push(component)
        }
    }
    pub(crate) fn clone_indices(&self) -> Vec<EntityId> {
        self.dense.clone()
    }
    pub fn try_update_pack(&mut self) -> Result<(), error::Pack>
    where
        T: 'static,
    {
        match self.pack_info.pack {
            Pack::NoPack => {
                self.pack_info.pack = Pack::Update(UpdatePack {
                    inserted: self.len(),
                    modified: 0,
                    deleted: Vec::new(),
                });
                Ok(())
            }
            Pack::Tight(_) => Err(error::Pack::AlreadyTightPack(TypeId::of::<T>())),
            Pack::Loose(_) => Err(error::Pack::AlreadyLoosePack(TypeId::of::<T>())),
            Pack::Update(_) => Err(error::Pack::AlreadyUpdatePack(TypeId::of::<T>())),
        }
    }
    pub fn update_pack(&mut self)
    where
        T: 'static,
    {
        self.try_update_pack().unwrap()
    }
    pub fn reserve(&mut self, additional: usize) {
        self.dense.reserve(additional);
        self.data.reserve(additional);
    }
    pub fn clear(&mut self) {
        if !self.is_unique() {
            match &mut self.pack_info.pack {
                Pack::Tight(tight) => tight.len = 0,
                Pack::Loose(loose) => loose.len = 0,
                Pack::Update(update) => update
                    .deleted
                    .extend(self.dense.drain(..).zip(self.data.drain(..))),
                Pack::NoPack => {}
            }
            self.dense.clear();
            self.data.clear();
        }
    }
}

impl<T> std::ops::Index<EntityId> for SparseSet<T> {
    type Output = T;
    fn index(&self, entity: EntityId) -> &Self::Output {
        self.get(entity).unwrap()
    }
}

impl<T> std::ops::IndexMut<EntityId> for SparseSet<T> {
    fn index_mut(&mut self, entity: EntityId) -> &mut Self::Output {
        self.get_mut(entity).unwrap()
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
    fn clear(&mut self) {
        <Self>::clear(self)
    }
    fn unpack(&mut self, entity: EntityId) {
        Self::unpack(self, entity);
    }
    fn any(&self) -> &dyn Any {
        self
    }
    fn any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[test]
fn insert() {
    let mut array = SparseSet::default();
    let mut entity_id = EntityId::zero();
    entity_id.set_index(0);
    assert!(array.insert("0", entity_id).is_none());
    entity_id.set_index(1);
    assert!(array.insert("1", entity_id).is_none());
    assert_eq!(array.len(), 2);
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), Some(&"0"));
    entity_id.set_index(1);
    assert_eq!(array.get(entity_id), Some(&"1"));
    entity_id.set_index(5);
    assert!(array.insert("5", entity_id).is_none());
    assert_eq!(array.get_mut(entity_id), Some(&mut "5"));
    entity_id.set_index(4);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(6);
    assert_eq!(array.get(entity_id), None);
    assert!(array.insert("6", entity_id).is_none());
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(6);
    assert_eq!(array.get_mut(entity_id), Some(&mut "6"));
    entity_id.set_index(4);
    assert_eq!(array.get(entity_id), None);
}
#[test]
fn remove() {
    let mut array = SparseSet::default();
    let mut entity_id = EntityId::zero();
    entity_id.set_index(0);
    array.insert("0", entity_id);
    entity_id.set_index(5);
    array.insert("5", entity_id);
    entity_id.set_index(10);
    array.insert("10", entity_id);
    entity_id.set_index(0);
    assert_eq!(array.actual_remove(entity_id), Some("0"));
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), Some(&"10"));
    assert_eq!(array.actual_remove(entity_id), Some("10"));
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), None);
    assert_eq!(array.len(), 1);
    entity_id.set_index(3);
    array.insert("3", entity_id);
    entity_id.set_index(10);
    array.insert("100", entity_id);
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(3);
    assert_eq!(array.get(entity_id), Some(&"3"));
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), Some(&"100"));
    entity_id.set_index(3);
    assert_eq!(array.actual_remove(entity_id), Some("3"));
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(3);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), Some(&"100"));
    assert_eq!(array.actual_remove(entity_id), Some("100"));
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(3);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.actual_remove(entity_id), Some("5"));
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(3);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), None);
    assert_eq!(array.len(), 0);
}
