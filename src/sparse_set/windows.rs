use super::{Pack, PackInfo};
use crate::error;
use crate::EntityId;
use core::ptr;
use std::ops::{Index, IndexMut};

pub struct Window<'a, T> {
    pub(crate) sparse: &'a [usize],
    pub(crate) dense: &'a [EntityId],
    pub(crate) data: &'a [T],
    pub(crate) pack_info: &'a PackInfo<T>,
}

impl<T> Clone for Window<'_, T> {
    fn clone(&self) -> Self {
        Window {
            sparse: self.sparse,
            dense: self.dense,
            data: self.data,
            pack_info: self.pack_info,
        }
    }
}

impl<T> Window<'_, T> {
    pub fn contains(&self, entity: EntityId) -> bool {
        entity.index() < self.sparse.len()
            && unsafe { *self.sparse.get_unchecked(entity.index()) } < self.dense.len()
            && unsafe {
                *self
                    .dense
                    .get_unchecked(*self.sparse.get_unchecked(entity.index()))
                    == entity
            }
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
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
    pub(crate) fn is_unique(&self) -> bool {
        self.sparse.is_empty() && self.dense.is_empty() && self.data.len() == 1
    }
}

impl<T> Index<EntityId> for Window<'_, T> {
    type Output = T;
    fn index(&self, entity: EntityId) -> &Self::Output {
        self.get(entity).unwrap()
    }
}

pub struct WindowMut<'a, T> {
    pub(crate) sparse: &'a mut [usize],
    pub(crate) dense: &'a mut [EntityId],
    pub(crate) data: &'a mut [T],
    pub(crate) pack_info: &'a mut PackInfo<T>,
}

impl<T> WindowMut<'_, T> {
    pub(crate) fn as_non_mut(&self) -> Window<'_, T> {
        Window {
            sparse: self.sparse,
            dense: self.dense,
            data: self.data,
            pack_info: self.pack_info,
        }
    }
    pub fn contains(&self, entity: EntityId) -> bool {
        self.as_non_mut().contains(entity)
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
        self.as_non_mut().len()
    }
    pub fn is_empty(&self) -> bool {
        self.as_non_mut().is_empty()
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
    pub(crate) fn pack(&mut self, entity: EntityId) {
        if self.contains(entity) {
            let dense_index = self.sparse[entity.index()];
            match &mut self.pack_info.pack {
                Pack::Tight(pack) => {
                    if dense_index >= pack.len {
                        self.sparse
                            .swap(self.dense[pack.len].index(), entity.index());
                        self.dense.swap(pack.len, dense_index);
                        self.data.swap(pack.len, dense_index);
                        pack.len += 1;
                    }
                }
                Pack::Loose(pack) => {
                    if dense_index >= pack.len {
                        self.sparse
                            .swap(self.dense[pack.len].index(), entity.index());
                        self.dense.swap(pack.len, dense_index);
                        self.data.swap(pack.len, dense_index);
                        pack.len += 1;
                    }
                }
                Pack::Update(_) => {}
                Pack::NoPack => {}
            }
        }
    }
    pub(crate) fn unpack(&mut self, entity: EntityId) {
        let dense_index = unsafe { *self.sparse.get_unchecked(entity.index()) };
        match &mut self.pack_info.pack {
            Pack::Tight(pack) => {
                if dense_index < pack.len {
                    pack.len -= 1;
                    // swap index and last packed element (can be the same)
                    unsafe {
                        self.sparse.swap(
                            self.dense.get_unchecked(pack.len).index(),
                            self.dense.get_unchecked(dense_index).index(),
                        )
                    };
                    self.dense.swap(dense_index, pack.len);
                    self.data.swap(dense_index, pack.len);
                }
            }
            Pack::Loose(pack) => {
                if dense_index < pack.len {
                    pack.len -= 1;
                    // swap index and last packed element (can be the same)
                    unsafe {
                        self.sparse.swap(
                            self.dense.get_unchecked(pack.len).index(),
                            self.dense.get_unchecked(dense_index).index(),
                        )
                    };
                    self.dense.swap(dense_index, pack.len);
                    self.data.swap(dense_index, pack.len);
                }
            }
            Pack::Update(_) => {}
            Pack::NoPack => {}
        }
    }
}

impl<T> Index<EntityId> for WindowMut<'_, T> {
    type Output = T;
    fn index(&self, entity: EntityId) -> &Self::Output {
        self.get(entity).unwrap()
    }
}

impl<T> IndexMut<EntityId> for WindowMut<'_, T> {
    fn index_mut(&mut self, entity: EntityId) -> &mut Self::Output {
        self.get_mut(entity).unwrap()
    }
}
/*
pub(crate) struct RawWindowMut<'a, T> {
    _phantom: PhantomData<&'a mut T>,
}
*/
