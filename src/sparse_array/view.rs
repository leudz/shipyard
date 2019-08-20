use super::PackInfo;
use crate::entity::Key;
use std::any::TypeId;

/// Immutable view into a `ComponentStorage`.
pub struct View<'a, T> {
    pub(crate) sparse: &'a [usize],
    pub(crate) dense: &'a [Key],
    pub(crate) data: &'a [T],
    pub(crate) pack_info: &'a PackInfo,
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
        self.dense.len()
    }
}

/// Mutable view into a `ComponentStorage`.
pub struct ViewMut<'a, T> {
    pub(crate) sparse: &'a mut Vec<usize>,
    pub(crate) dense: &'a mut Vec<Key>,
    pub(crate) data: &'a mut Vec<T>,
    pub(crate) pack_info: &'a mut PackInfo,
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
            match self.pack_info {
                PackInfo::Tight(pack_info) => {
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
                PackInfo::Loose(pack_info) => {
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
                PackInfo::NoPack => {}
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
        self.dense.len()
    }
    /// Consumes the ViewMut and returns a ViewSemiMut.
    pub(crate) fn into_raw(self) -> RawViewMut<'a, T> {
        RawViewMut {
            sparse: self.sparse,
            dense: self.dense,
            data: self.data.as_mut_ptr(),
        }
    }
    /// Borrows the ViewMut and returns a ViewSemiMut.
    pub(crate) fn raw(&mut self) -> RawViewMut<T> {
        RawViewMut {
            sparse: self.sparse,
            dense: self.dense,
            data: self.data.as_mut_ptr(),
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
            match self.pack_info {
                PackInfo::Tight(pack_info) => {
                    if dense_index >= pack_info.len {
                        self.sparse
                            .swap(self.dense[pack_info.len].index(), entity.index());
                        self.dense.swap(pack_info.len, dense_index);
                        self.data.swap(pack_info.len, dense_index);
                        pack_info.len += 1;
                    }
                }
                _ => {}
            }
        }
    }
    pub(crate) fn unpack(&mut self, entity: Key) {
        let dense_index = unsafe { *self.sparse.get_unchecked(entity.index()) };
        match self.pack_info {
            PackInfo::Tight(pack_info) => {
                let pack_len = pack_info.len;
                if dense_index < pack_len {
                    pack_info.len -= 1;
                    // swap index and last packed element (can be the same)
                    unsafe {
                        *self
                            .sparse
                            .get_unchecked_mut(self.dense.get_unchecked(pack_len - 1).index()) =
                            dense_index;
                    }
                    self.dense.swap(dense_index, pack_len - 1);
                    self.data.swap(dense_index, pack_len - 1);
                }
            }
            PackInfo::Loose(pack_info) => {
                let pack_len = pack_info.len;
                if dense_index < pack_len {
                    pack_info.len -= 1;
                    // swap index and last packed element (can be the same)
                    unsafe {
                        *self
                            .sparse
                            .get_unchecked_mut(self.dense.get_unchecked(pack_len - 1).index()) =
                            dense_index;
                    }
                    self.dense.swap(dense_index, pack_len - 1);
                    self.data.swap(dense_index, pack_len - 1);
                }
            }
            PackInfo::NoPack => {}
        }
    }
}

// Used in iterators to be able to keep a pointer to the indices
pub struct RawViewMut<'a, T> {
    pub(crate) sparse: &'a [usize],
    pub(crate) dense: &'a [Key],
    pub(crate) data: *mut T,
}

unsafe impl<T: Send + Sync> Send for RawViewMut<'_, T> {}

impl<'a, T> RawViewMut<'a, T> {
    /// Returns true if the `entity` has this component.
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
    /// Returns the number of components in the view.
    pub fn len(&self) -> usize {
        self.dense.len()
    }
}

impl<'a, T> Clone for RawViewMut<'a, T> {
    fn clone(&self) -> Self {
        RawViewMut {
            sparse: self.sparse,
            dense: self.dense,
            data: self.data,
        }
    }
}
