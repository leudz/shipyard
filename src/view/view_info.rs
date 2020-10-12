use super::ViewMut;
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
use tinyvec::TinyVec;

/// Used by [`AddEntity`] to list the storage composing an entity.
pub trait ViewInfo {
    fn storage_info(info: &mut TinyVec<[StorageId; 10]>);
}

impl<T: 'static> ViewInfo for ViewMut<'_, T> {
    fn storage_info(info: &mut TinyVec<[StorageId; 10]>) {
        info.push(StorageId::of::<SparseSet<T>>());
    }
}

impl<T: 'static> ViewInfo for &mut ViewMut<'_, T> {
    fn storage_info(info: &mut TinyVec<[StorageId; 10]>) {
        info.push(StorageId::of::<SparseSet<T>>());
    }
}
