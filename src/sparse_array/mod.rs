mod pack_info;
mod read_write;
mod view;
mod view_add_entity;

use crate::entity::Key;
use pack_info::PackInfo;
pub(crate) use read_write::{Read, Write};
use std::any::TypeId;
use std::sync::Arc;
pub(crate) use view::{RawViewMut, View, ViewMut};
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
    dense: Vec<Key>,
    data: Vec<T>,
    pack_info: PackInfo,
}

impl<T> Default for SparseArray<T> {
    fn default() -> Self {
        SparseArray {
            sparse: Vec::new(),
            dense: Vec::new(),
            data: Vec::new(),
            pack_info: Default::default(),
        }
    }
}

impl<T> SparseArray<T> {
    /// Inserts a value at a given index, if a value was already present it will be returned.
    pub(crate) fn insert(&mut self, value: T, entity: Key) -> Option<T> {
        self.view_mut().insert(value, entity)
    }
    /// Returns true if the sparse array contains data at this index.
    pub(crate) fn contains(&self, entity: Key) -> bool {
        self.view().contains(entity)
    }
    /// Returns a reference to the element at this index if present.
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
    /// Returns a mutable reference to the element at this index if present.
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
    /// Removes and returns the element at index if present.
    pub(crate) fn remove(&mut self, entity: Key) -> Option<T> {
        self.view_mut().remove(entity)
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
    pub(crate) fn pack(&mut self, entity: Key) {
        self.view_mut().pack(entity)
    }
    pub(crate) fn is_packed_owned(&self) -> bool {
        !self.pack_info.owned_type.is_empty()
    }
    pub(crate) fn pack_types_owned(&self) -> &[TypeId] {
        &self.pack_info.owned_type
    }
    pub(crate) fn clone_indices(&self) -> Vec<Key> {
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
        self.get(index).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn insert() {
        let mut array = SparseArray::default();
        let mut key = Key::zero();
        key.set_index(0);
        assert!(array.insert("0", key).is_none());
        key.set_index(1);
        assert!(array.insert("1", key).is_none());
        assert_eq!(array.len(), 2);
        key.set_index(0);
        assert_eq!(array.get(key), Some(&"0"));
        key.set_index(1);
        assert_eq!(array.get(key), Some(&"1"));
        key.set_index(5);
        assert!(array.insert("5", key).is_none());
        assert_eq!(array.get_mut(key), Some(&mut "5"));
        key.set_index(4);
        assert_eq!(array.get(key), None);
        key.set_index(6);
        assert_eq!(array.get(key), None);
        assert!(array.insert("6", key).is_none());
        key.set_index(5);
        assert_eq!(array.get(key), Some(&"5"));
        key.set_index(6);
        assert_eq!(array.get_mut(key), Some(&mut "6"));
        key.set_index(4);
        assert_eq!(array.get(key), None);
    }
    #[test]
    fn remove() {
        let mut array = SparseArray::default();
        let mut key = Key::zero();
        key.set_index(0);
        array.insert("0", key);
        key.set_index(5);
        array.insert("5", key);
        key.set_index(10);
        array.insert("10", key);
        key.set_index(0);
        assert_eq!(array.remove(key), Some("0"));
        assert_eq!(array.get(key), None);
        key.set_index(5);
        assert_eq!(array.get(key), Some(&"5"));
        key.set_index(10);
        assert_eq!(array.get(key), Some(&"10"));
        assert_eq!(array.remove(key), Some("10"));
        key.set_index(0);
        assert_eq!(array.get(key), None);
        key.set_index(5);
        assert_eq!(array.get(key), Some(&"5"));
        key.set_index(10);
        assert_eq!(array.get(key), None);
        assert_eq!(array.len(), 1);
        key.set_index(3);
        array.insert("3", key);
        key.set_index(10);
        array.insert("100", key);
        key.set_index(0);
        assert_eq!(array.get(key), None);
        key.set_index(3);
        assert_eq!(array.get(key), Some(&"3"));
        key.set_index(5);
        assert_eq!(array.get(key), Some(&"5"));
        key.set_index(10);
        assert_eq!(array.get(key), Some(&"100"));
        key.set_index(3);
        assert_eq!(array.remove(key), Some("3"));
        key.set_index(0);
        assert_eq!(array.get(key), None);
        key.set_index(3);
        assert_eq!(array.get(key), None);
        key.set_index(5);
        assert_eq!(array.get(key), Some(&"5"));
        key.set_index(10);
        assert_eq!(array.get(key), Some(&"100"));
        assert_eq!(array.remove(key), Some("100"));
        key.set_index(0);
        assert_eq!(array.get(key), None);
        key.set_index(3);
        assert_eq!(array.get(key), None);
        key.set_index(5);
        assert_eq!(array.get(key), Some(&"5"));
        key.set_index(10);
        assert_eq!(array.get(key), None);
        key.set_index(5);
        assert_eq!(array.remove(key), Some("5"));
        key.set_index(0);
        assert_eq!(array.get(key), None);
        key.set_index(3);
        assert_eq!(array.get(key), None);
        key.set_index(5);
        assert_eq!(array.get(key), None);
        key.set_index(10);
        assert_eq!(array.get(key), None);
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
