mod pack_info;
mod read_write;
mod view;
mod view_add_entity;

use crate::entity::Key;
use crate::remove::RemoveComponent;
use pack_info::PackInfo;
pub(crate) use read_write::{Read, Write};
use std::any::TypeId;
use std::sync::Arc;
pub(crate) use view::{View, ViewMut, ViewSemiMut};
pub(crate) use view_add_entity::ViewAddEntity;

// A sparse array is a data structure with 2 vectors: one sparse, the other dense.
// Only usize can be added. On insertion, the number is pushed into the dense vector
// and sparse[number] is set to dense.len() - 1.
// For all number present in the sparse array, dense[sparse[number]] == number.
// For all other values if set sparse[number] will have any value left there
// and if set dense[sparse[number]] != number.
// We can't be limited to store solely integers, this is why there is a third vector.
// It mimics the dense vector in regard to insertion/deletion.
pub struct SparseArray<T> {
    sparse: Vec<usize>,
    dense: Vec<usize>,
    data: Vec<T>,
    remove_function: usize,
    pack_info: PackInfo,
}

impl<T> Default for SparseArray<T> {
    fn default() -> Self {
        let mut sparse_array = SparseArray {
            sparse: Vec::new(),
            dense: Vec::new(),
            data: Vec::new(),
            remove_function: 0,
            pack_info: Default::default(),
        };
        let ptr: [usize; 2] = unsafe {
            std::ptr::read(&sparse_array as &dyn RemoveComponent as *const _ as *const _)
        };
        sparse_array.remove_function = ptr[1];
        sparse_array
    }
}

impl<T> SparseArray<T> {
    /// Inserts a value at a given index, if a value was already present it will be returned.
    pub(crate) fn insert(&mut self, value: T, index: usize) -> Option<T> {
        self.view_mut().insert(value, index)
    }
    /// Returns true if the sparse array contains data at this index.
    pub(crate) fn contains(&self, index: usize) -> bool {
        self.view().contains_index(index)
    }
    /// Returns a reference to the element at this index if present.
    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        if self.contains(index) {
            Some(unsafe { self.data.get_unchecked(*self.sparse.get_unchecked(index)) })
        } else {
            None
        }
    }
    /// Returns a mutable reference to the element at this index if present.
    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if self.contains(index) {
            Some(unsafe {
                self.data
                    .get_unchecked_mut(*self.sparse.get_unchecked(index))
            })
        } else {
            None
        }
    }
    /// Removes and returns the element at index if present.
    pub(crate) fn remove(&mut self, index: usize) -> Option<T> {
        self.view_mut().remove(index)
    }
    /// Returns the number of element present in the sparse array.
    pub fn len(&self) -> usize {
        self.dense.len()
    }
    pub(crate) fn view(&self) -> View<T> {
        View {
            sparse: &self.sparse,
            dense: &self.dense,
            data: &self.data,
            pack_info: &self.pack_info,
        }
    }
    pub(crate) fn view_mut(&mut self) -> ViewMut<T> {
        ViewMut {
            sparse: &mut self.sparse,
            dense: &mut self.dense,
            data: &mut self.data,
            pack_info: &mut self.pack_info,
        }
    }
    //          ▼ old end of pack
    //              ▼ new end of pack
    // [_ _ _ _ | _ | _ _ _ _ _]
    //            ▲       ▼
    //            ---------
    //              pack
    pub(crate) fn pack(&mut self, index: usize) {
        self.view_mut().pack(index)
    }
    pub(crate) fn is_packed_owned(&self) -> bool {
        !self.pack_info.owned_type.is_empty()
    }
    pub(crate) fn pack_types_owned(&self) -> &[TypeId] {
        &self.pack_info.owned_type
    }
    pub(crate) fn clone_indices(&self) -> Vec<usize> {
        self.dense.clone()
    }
    pub(crate) fn pack_with(&mut self, types: Arc<[TypeId]>) {
        self.pack_info.owned_type = types;
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
}

impl<T> std::ops::Index<Key> for SparseArray<T> {
    type Output = T;
    fn index(&self, index: Key) -> &Self::Output {
        self.get(index.index()).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn insert() {
        let mut array = SparseArray::default();
        array.insert("0", 0);
        array.insert("1", 1);
        assert_eq!(array.len(), 2);
        assert_eq!(array.get(0), Some(&"0"));
        assert_eq!(array.get(1), Some(&"1"));
        array.insert("5", 5);
        assert_eq!(array.get_mut(5), Some(&mut "5"));
        assert_eq!(array.get(4), None);
        assert_eq!(array.get(6), None);
        array.insert("6", 6);
        assert_eq!(array.get(5), Some(&"5"));
        assert_eq!(array.get_mut(6), Some(&mut "6"));
        assert_eq!(array.get(4), None);
    }
    #[test]
    fn remove() {
        let mut array = SparseArray::default();
        array.insert("0", 0);
        array.insert("5", 5);
        array.insert("10", 10);
        assert_eq!(array.remove(0), Some("0"));
        assert_eq!(array.get(0), None);
        assert_eq!(array.get(5), Some(&"5"));
        assert_eq!(array.get(10), Some(&"10"));
        assert_eq!(array.remove(10), Some("10"));
        assert_eq!(array.get(0), None);
        assert_eq!(array.get(5), Some(&"5"));
        assert_eq!(array.get(10), None);
        assert_eq!(array.len(), 1);
        array.insert("3", 3);
        array.insert("100", 10);
        assert_eq!(array.get(0), None);
        assert_eq!(array.get(3), Some(&"3"));
        assert_eq!(array.get(5), Some(&"5"));
        assert_eq!(array.get(10), Some(&"100"));
        assert_eq!(array.remove(3), Some("3"));
        assert_eq!(array.get(0), None);
        assert_eq!(array.get(3), None);
        assert_eq!(array.get(5), Some(&"5"));
        assert_eq!(array.get(10), Some(&"100"));
        assert_eq!(array.remove(10), Some("100"));
        assert_eq!(array.get(0), None);
        assert_eq!(array.get(3), None);
        assert_eq!(array.get(5), Some(&"5"));
        assert_eq!(array.get(10), None);
        assert_eq!(array.remove(5), Some("5"));
        assert_eq!(array.get(0), None);
        assert_eq!(array.get(3), None);
        assert_eq!(array.get(5), None);
        assert_eq!(array.get(10), None);
        assert_eq!(array.len(), 0);
    }
    #[test]
    fn check_types_for_pack_owned() {
        let mut pack_types = vec![
            TypeId::of::<u32>(),
            TypeId::of::<usize>(),
            TypeId::of::<u8>(),
        ];
        pack_types.sort_unstable();
        let mut sparse_array = SparseArray::<u32>::default();
        sparse_array.pack_with(pack_types.into_boxed_slice().into());

        let mut slice = [
            TypeId::of::<u32>(),
            TypeId::of::<usize>(),
            TypeId::of::<u8>(),
        ];
        slice.sort_unstable();
        assert!(!sparse_array.should_pack_owned(&slice).is_empty());

        let mut pack_types = vec![TypeId::of::<usize>(), TypeId::of::<u8>()];
        pack_types.sort_unstable();
        sparse_array.pack_with(pack_types.into_boxed_slice().into());
        assert!(!sparse_array.should_pack_owned(&slice).is_empty());

        let mut slice = [TypeId::of::<u32>(), TypeId::of::<u8>()];
        slice.sort_unstable();
        let mut pack_types = vec![
            TypeId::of::<u32>(),
            TypeId::of::<usize>(),
            TypeId::of::<u8>(),
        ];
        pack_types.sort_unstable();
        sparse_array.pack_with(pack_types.into_boxed_slice().into());
        assert!(sparse_array.should_pack_owned(&slice).is_empty());
    }
}
