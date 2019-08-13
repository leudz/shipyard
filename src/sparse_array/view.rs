use super::PackInfo;
use std::any::TypeId;

/// Immutable view into a `ComponentStorage`.
pub struct View<'a, T> {
    pub(crate) sparse: &'a [usize],
    pub(crate) dense: &'a [usize],
    pub(crate) data: &'a [T],
    pub(super) pack_info: &'a PackInfo,
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
    pub(crate) fn contains_index(&self, index: usize) -> bool {
        index < self.sparse.len()
            && unsafe { *self.sparse.get_unchecked(index) } < self.dense.len()
            && unsafe { *self.dense.get_unchecked(*self.sparse.get_unchecked(index)) == index }
    }
    /// Returns a reference to the component if the `entity` has it.
    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        if self.contains_index(index) {
            Some(unsafe { self.data.get_unchecked(*self.sparse.get_unchecked(index)) })
        } else {
            None
        }
    }
    /// Returns the number of components in the view.
    pub fn len(&self) -> usize {
        self.dense.len()
    }
    /// Returns a slice of the types packed with this one.
    pub(crate) fn pack_types_owned(&self) -> &[TypeId] {
        &self.pack_info.owned_type
    }
    /// Returns the length of the packed area within the data array.
    pub(crate) fn pack_len(&self) -> usize {
        self.pack_info.owned_len
    }
}

/// Mutable view into a `ComponentStorage`.
pub struct ViewMut<'a, T> {
    pub(crate) sparse: &'a mut Vec<usize>,
    pub(crate) dense: &'a mut Vec<usize>,
    pub(crate) data: &'a mut Vec<T>,
    pub(super) pack_info: &'a mut PackInfo,
}

impl<'a, T> ViewMut<'a, T> {
    /// Add the component to the `entity`.
    pub(crate) fn insert(&mut self, mut value: T, index: usize) -> Option<T> {
        if index >= self.sparse.len() {
            self.sparse.resize(index + 1, 0);
        }
        if let Some(data) = self.get_mut(index) {
            std::mem::swap(data, &mut value);
            Some(value)
        } else {
            unsafe { *self.sparse.get_unchecked_mut(index) = self.dense.len() };
            self.dense.push(index);
            self.data.push(value);
            None
        }
    }
    pub(crate) fn contains_index(&self, index: usize) -> bool {
        index < self.sparse.len()
            && unsafe { *self.sparse.get_unchecked(index) } < self.dense.len()
            && unsafe { *self.dense.get_unchecked(*self.sparse.get_unchecked(index)) == index }
    }
    /// Returns a reference to the component if the `entity` has it.
    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        if self.contains_index(index) {
            Some(unsafe { self.data.get_unchecked(*self.sparse.get_unchecked(index)) })
        } else {
            None
        }
    }
    /// Returns a mutable reference to the component if the `entity` has it.
    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if self.contains_index(index) {
            Some(unsafe {
                self.data
                    .get_unchecked_mut(*self.sparse.get_unchecked(index))
            })
        } else {
            None
        }
    }
    /// Remove the component if the `entity` has it and returns it.
    pub(crate) fn remove(&mut self, index: usize) -> Option<T> {
        if self.contains_index(index) {
            let mut dense_index = unsafe { *self.sparse.get_unchecked(index) };
            let pack_len = self.pack_len();
            if dense_index < pack_len {
                self.pack_info.owned_len -= 1;
                // swap index and last packed element (can be the same)
                unsafe {
                    *self
                        .sparse
                        .get_unchecked_mut(*self.dense.get_unchecked(pack_len - 1)) = dense_index;
                }
                self.dense.swap(dense_index, pack_len - 1);
                self.data.swap(dense_index, pack_len - 1);
                dense_index = pack_len - 1;
            }
            unsafe {
                *self
                    .sparse
                    .get_unchecked_mut(*self.dense.get_unchecked(self.dense.len() - 1)) =
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
    pub(crate) fn non_mut(&self) -> View<T> {
        View {
            sparse: self.sparse,
            dense: self.dense,
            data: self.data,
            pack_info: self.pack_info,
        }
    }
    pub(crate) fn pack(&mut self, index: usize) {
        if self.contains_index(index) {
            let dense_index = self.sparse[index];
            if dense_index >= self.pack_info.owned_len {
                self.sparse
                    .swap(self.dense[self.pack_info.owned_len], index);
                self.dense.swap(self.pack_info.owned_len, dense_index);
                self.data.swap(self.pack_info.owned_len, dense_index);
                self.pack_info.owned_len += 1;
            }
        }
    }
    /// Returns a slice of the types packed with this one.
    pub(crate) fn pack_types_owned(&self) -> &[TypeId] {
        &self.pack_info.owned_type
    }
    /// Returns true if is packed with any other type.
    pub(crate) fn is_packed_owned(&self) -> bool {
        !self.pack_info.owned_type.is_empty()
    }
    /// Check if `slice` has all the necessary types to be packed.
    /// Assumes `slice` is sorted and don't have any duplicate.
    pub(crate) fn should_pack_owned(&self, slice: &[TypeId]) -> &[TypeId] {
        let pack_types = self.pack_types_owned();
        let mut i = 0;
        let mut j = 0;

        while i < pack_types.len() && j < slice.len() {
            if pack_types[i] == slice[j] {
                i += 1;
                j += 1;
            } else if pack_types[i] > slice[j] {
                j += 1;
            } else {
                return &[];
            }
        }

        if i == pack_types.len() && j == slice.len() {
            pack_types
        } else {
            &[]
        }
    }
    /// Returns the length of the packed area within the data array.
    pub(crate) fn pack_len(&self) -> usize {
        self.pack_info.owned_len
    }
}

// Used in iterators to be able to keep a pointer to the indices
pub struct RawViewMut<'a, T> {
    pub(crate) sparse: &'a [usize],
    pub(crate) dense: &'a [usize],
    pub(crate) data: *mut T,
}

unsafe impl<T: Send + Sync> Send for RawViewMut<'_, T> {}

impl<'a, T> RawViewMut<'a, T> {
    /// Returns true if the `entity` has this component.
    pub(crate) fn contains(&self, index: usize) -> bool {
        index < self.sparse.len()
            && unsafe { *self.sparse.get_unchecked(index) } < self.dense.len()
            && unsafe { *self.dense.get_unchecked(*self.sparse.get_unchecked(index)) == index }
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
