mod read_write;
mod view;

use crate::entity::Key;
pub(crate) use read_write::{Read, Write};
pub(crate) use view::{View, ViewMut};

/* A sparse array is a data structure with 2 vectors: one sparse, the other dense.
 * Only usize can be added. On insertion, the number is pushed into the dense vector
 * and sparse[number] is set to dense.len() - 1.
 * For all number present in the sparse array, dense[sparse[number]] == number.
 * For all other values if set sparse[number] will have any value left there
 * and if set dense[sparse[number]] != number.
 * We can't be limited to store solely integers, this is why there is a third vector.
 * It mimics the dense vector in regard to insertion/deletion.
*/
pub struct SparseArray<T> {
    sparse: Vec<usize>,
    dense: Vec<usize>,
    data: Vec<T>,
}

impl<T> Default for SparseArray<T> {
    fn default() -> Self {
        SparseArray {
            sparse: Vec::new(),
            dense: Vec::new(),
            data: Vec::new(),
        }
    }
}

impl<T> SparseArray<T> {
    /// Inserts a value at a given index, if a value was already present it will be returned.
    pub(crate) fn insert(&mut self, index: usize, mut value: T) -> Option<T> {
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
    /// Returns true if the sparse array contains data at this index.
    fn contains(&self, index: usize) -> bool {
        index < self.sparse.len()
            && unsafe { *self.sparse.get_unchecked(index) } < self.dense.len()
            && unsafe { *self.dense.get_unchecked(*self.sparse.get_unchecked(index)) == index }
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
        if self.contains(index) {
            let dense_index = unsafe { *self.sparse.get_unchecked(index) };
            unsafe {
                *self
                    .sparse
                    .get_unchecked_mut(*self.dense.get_unchecked(self.dense.len() - 1)) =
                    dense_index
            };
            self.dense.swap_remove(dense_index);
            Some(self.data.swap_remove(dense_index))
        } else {
            None
        }
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
        }
    }
    pub(crate) fn view_mut(&mut self) -> ViewMut<T> {
        ViewMut {
            sparse: &mut self.sparse,
            dense: &mut self.dense,
            data: &mut self.data,
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
        array.insert(0, "0");
        array.insert(1, "1");
        assert_eq!(array.len(), 2);
        assert_eq!(array.get(0), Some(&"0"));
        assert_eq!(array.get(1), Some(&"1"));
        array.insert(5, "5");
        assert_eq!(array.get_mut(5), Some(&mut "5"));
        assert_eq!(array.get(4), None);
        assert_eq!(array.get(6), None);
        array.insert(6, "6");
        assert_eq!(array.get(5), Some(&"5"));
        assert_eq!(array.get_mut(6), Some(&mut "6"));
        assert_eq!(array.get(4), None);
    }
    #[test]
    fn remove() {
        let mut array = SparseArray::default();
        array.insert(0, "0");
        array.insert(5, "5");
        array.insert(10, "10");
        assert_eq!(array.remove(0), Some("0"));
        assert_eq!(array.get(0), None);
        assert_eq!(array.get(5), Some(&"5"));
        assert_eq!(array.get(10), Some(&"10"));
        assert_eq!(array.remove(10), Some("10"));
        assert_eq!(array.get(0), None);
        assert_eq!(array.get(5), Some(&"5"));
        assert_eq!(array.get(10), None);
        assert_eq!(array.len(), 1);
        array.insert(3, "3");
        array.insert(10, "100");
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
}
