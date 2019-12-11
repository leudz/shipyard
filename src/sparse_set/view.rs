use super::{Pack, PackInfo};
use crate::entity::Key;
use std::marker::PhantomData;

/// Immutable view into a `Storage`.
pub struct View<'a, T> {
    pub(crate) sparse: &'a [usize],
    pub(crate) dense: &'a [Key],
    pub(crate) data: &'a [T],
    pub(crate) pack_info: &'a PackInfo<T>,
}

impl<'a, T> Clone for View<'a, T> {
    fn clone(&self) -> Self {
        View {
            sparse: self.sparse,
            dense: self.dense,
            data: self.data,
            pack_info: self.pack_info,
        }
    }
}

impl<T> View<'_, T> {
    pub(crate) fn contains(&self, entity: Key) -> bool {
        entity.index() < self.sparse.len()
            && unsafe { *self.sparse.get_unchecked(entity.index()) } < self.dense.len()
            && unsafe {
                *self
                    .dense
                    .get_unchecked(*self.sparse.get_unchecked(entity.index()))
                    == entity
            }
    }
    /// Returns a reference to the component if the `entity` has it.
    pub(crate) fn get(&self, entity: Key) -> Option<&T> {
        if self.contains(entity) {
            Some(unsafe {
                self.data
                    .get_unchecked(*self.sparse.get_unchecked(entity.index()))
            })
        } else {
            None
        }
    }
    /// Returns the number of components in the view.
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    pub fn modified(&self) -> View<T> {
        match &self.pack_info.pack {
            Pack::Update(pack) => View {
                sparse: self.sparse,
                dense: &self.dense[pack.inserted..pack.inserted + pack.modified],
                data: &self.data[pack.inserted..pack.inserted + pack.modified],
                pack_info: self.pack_info,
            },
            _ => View {
                sparse: &[],
                dense: &[],
                data: &[],
                pack_info: self.pack_info,
            },
        }
    }
    pub fn inserted(&self) -> View<T> {
        match &self.pack_info.pack {
            Pack::Update(pack) => View {
                sparse: self.sparse,
                dense: &self.dense[0..pack.inserted],
                data: &self.data[0..pack.inserted],
                pack_info: self.pack_info,
            },
            _ => View {
                sparse: &[],
                dense: &[],
                data: &[],
                pack_info: self.pack_info,
            },
        }
    }
}

/// Mutable view into a `Storage`.
pub struct ViewMut<'a, T> {
    pub(crate) sparse: &'a mut Vec<usize>,
    pub(crate) dense: &'a mut Vec<Key>,
    pub(crate) data: &'a mut Vec<T>,
    pub(crate) pack_info: &'a mut PackInfo<T>,
}

impl<'a, T: 'static> ViewMut<'a, T> {
    /// Add the component to the `entity`.
    pub(crate) fn insert(&mut self, mut value: T, entity: Key) -> Option<T> {
        if entity.index() >= self.sparse.len() {
            self.sparse.resize(entity.index() + 1, 0);
        }
        if let Some(data) = self.get_mut(entity) {
            std::mem::swap(data, &mut value);
            Some(value)
        } else {
            unsafe { *self.sparse.get_unchecked_mut(entity.index()) = self.dense.len() };
            self.dense.push(entity);
            self.data.push(value);
            None
        }
    }
    pub(crate) fn contains(&self, entity: Key) -> bool {
        self.as_non_mut().contains(entity)
    }
    /// Returns a reference to the component if the `entity` has it.
    pub(crate) fn get(&self, entity: Key) -> Option<&T> {
        if self.contains(entity) {
            Some(unsafe {
                self.data
                    .get_unchecked(*self.sparse.get_unchecked(entity.index()))
            })
        } else {
            None
        }
    }
    /// Returns a mutable reference to the component if the `entity` has it.
    pub(crate) fn get_mut(&mut self, entity: Key) -> Option<&mut T> {
        if self.contains(entity) {
            Some(unsafe {
                self.data
                    .get_unchecked_mut(*self.sparse.get_unchecked(entity.index()))
            })
        } else {
            None
        }
    }
    /// Remove the component if the `entity` has it and returns it.
    pub(crate) fn remove(&mut self, entity: Key) -> Option<T> {
        if self.contains(entity) {
            let mut dense_index = unsafe { *self.sparse.get_unchecked(entity.index()) };
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
                *self
                    .sparse
                    .get_unchecked_mut(self.dense.get_unchecked(self.dense.len() - 1).index()) =
                    dense_index;
            }
            self.dense.swap_remove(dense_index);
            Some(self.data.swap_remove(dense_index))
        } else {
            None
        }
    }
    /// Returns the number of components in the view.
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    /// Consumes the ViewMut and returns a RawViewMut.
    pub(crate) fn into_raw(self) -> RawViewMut<'a, T> {
        RawViewMut {
            sparse: self.sparse.as_mut_ptr(),
            dense: self.dense.as_mut_ptr(),
            data: self.data.as_mut_ptr(),
            sparse_len: self.sparse.len(),
            len: self.dense.len(),
            pack_info: self.pack_info,
            _phantom: PhantomData,
        }
    }
    /// Borrows the ViewMut and returns a RawViewMut.
    pub(crate) fn raw(&mut self) -> RawViewMut<T> {
        RawViewMut {
            sparse: self.sparse.as_mut_ptr(),
            dense: self.dense.as_mut_ptr(),
            data: self.data.as_mut_ptr(),
            sparse_len: self.sparse.len(),
            len: self.dense.len(),
            pack_info: self.pack_info,
            _phantom: PhantomData,
        }
    }
    /// Borrows the ViewMut and returns a View.
    pub(crate) fn as_non_mut(&self) -> View<T> {
        View {
            sparse: self.sparse,
            dense: self.dense,
            data: self.data,
            pack_info: self.pack_info,
        }
    }
    pub(crate) fn pack(&mut self, entity: Key) {
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
    pub(crate) fn unpack(&mut self, entity: Key) {
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
    pub fn modified(&self) -> View<T> {
        match &self.pack_info.pack {
            Pack::Update(pack) => View {
                sparse: self.sparse,
                dense: &self.dense[pack.inserted..pack.inserted + pack.modified],
                data: &self.data[pack.inserted..pack.inserted + pack.modified],
                pack_info: self.pack_info,
            },
            _ => View {
                sparse: &[],
                dense: &[],
                data: &[],
                pack_info: self.pack_info,
            },
        }
    }
    pub fn modified_mut(&mut self) -> RawViewMut<T> {
        match &self.pack_info.pack {
            Pack::Update(pack) => {
                if self.dense.len() >= pack.inserted + pack.modified {
                    let new = pack.inserted;
                    let modified = pack.modified;
                    let mut raw = self.raw();
                    raw.dense = unsafe { raw.dense.add(new) };
                    raw.data = unsafe { raw.data.add(new) };
                    raw.len = modified;
                    raw
                } else {
                    let mut raw = self.raw();
                    raw.len = 0;
                    raw
                }
            }
            _ => {
                let mut raw = self.raw();
                raw.len = 0;
                raw
            }
        }
    }
    pub fn inserted(&self) -> View<T> {
        match &self.pack_info.pack {
            Pack::Update(pack) => View {
                sparse: self.sparse,
                dense: &self.dense[0..pack.inserted],
                data: &self.data[0..pack.inserted],
                pack_info: self.pack_info,
            },
            _ => View {
                sparse: &[],
                dense: &[],
                data: &[],
                pack_info: self.pack_info,
            },
        }
    }
    pub fn inserted_mut(&mut self) -> RawViewMut<T> {
        match &self.pack_info.pack {
            Pack::Update(pack) => {
                if self.dense.len() >= pack.inserted {
                    let new = pack.inserted;
                    let mut raw = self.raw();
                    raw.len = new;
                    raw
                } else {
                    let mut raw = self.raw();
                    raw.len = 0;
                    raw
                }
            }
            _ => {
                let mut raw = self.raw();
                raw.len = 0;
                raw
            }
        }
    }
    pub fn take_removed(&mut self) -> Option<Vec<(Key, T)>> {
        match &mut self.pack_info.pack {
            Pack::Update(pack) => {
                let mut vec = Vec::with_capacity(pack.removed.capacity());
                std::mem::swap(&mut vec, &mut pack.removed);
                Some(vec)
            }
            _ => None,
        }
    }
    pub fn clear_modified(&mut self) {
        if let Pack::Update(pack) = &mut self.pack_info.pack {
            pack.modified = 0;
        }
    }
    /// If you intent to clear both modified and new, starting by modified will be more efficient.
    pub fn clear_inserted(&mut self) {
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
        }
    }
}

// Used in iterators
pub struct RawViewMut<'a, T> {
    pub(crate) sparse: *mut usize,
    pub(crate) sparse_len: usize,
    pub(crate) dense: *mut Key,
    pub(crate) len: usize,
    pub(crate) data: *mut T,
    pub(crate) pack_info: *mut PackInfo<T>,
    _phantom: PhantomData<&'a ()>,
}

unsafe impl<T: Send + Sync> Send for RawViewMut<'_, T> {}

impl<'a, T> RawViewMut<'a, T> {
    pub(crate) unsafe fn contains(&self, entity: Key) -> bool {
        entity.index() < self.sparse_len
            && *self.sparse.add(entity.index()) < self.len
            && *self.dense.add(*self.sparse.add(entity.index())) == entity
    }
}

impl<'a, T> Clone for RawViewMut<'a, T> {
    fn clone(&self) -> Self {
        RawViewMut {
            sparse: self.sparse,
            dense: self.dense,
            data: self.data,
            sparse_len: self.sparse_len,
            len: self.len,
            pack_info: self.pack_info,
            _phantom: PhantomData,
        }
    }
}
