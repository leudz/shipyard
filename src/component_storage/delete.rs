use crate::sparse_array::SparseArray;

// Trait used to delete components inside a `SparseArray<T>`
// without knowing what `T` is.
pub(super) trait Delete {
    fn delete(&mut self, index: usize);
}

impl<T> Delete for SparseArray<T> {
    fn delete(&mut self, index: usize) {
        self.remove(index);
    }
}
