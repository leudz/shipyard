mod component;
mod entity;

use crate::sparse_array::SparseArray;
pub use component::Remove;

pub(crate) trait RemoveComponent {
    fn remove_component(&mut self, index: usize);
}

impl<T> RemoveComponent for SparseArray<T> {
    fn remove_component(&mut self, index: usize) {
        self.remove(index);
    }
}
