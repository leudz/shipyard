use crate::not::Not;
use crate::sparse_set::{Pack, RawViewMut, View};
use crate::storage::EntityId;

// Abstracts different types of view to iterate over
// mutable and immutable views with the same iterator
#[doc(hidden)]
pub trait AbstractMut: Clone + Send {
    type Out;
    type Slice;
    // # Safety
    // The lifetime has to be valid
    unsafe fn get_data(&mut self, index: usize) -> Self::Out;
    // # Safety
    // The lifetime has to be valid
    unsafe fn get_data_slice(&mut self, indices: std::ops::Range<usize>) -> Self::Slice;
    fn indices(&self) -> *const EntityId;
    unsafe fn mark_modified(&mut self, index: usize) -> Self::Out;
    unsafe fn mark_id_modified(&mut self, entity: EntityId) -> Self::Out;
    unsafe fn id_at(&self, index: usize) -> EntityId;
    fn index_of(&self, entity: EntityId) -> Option<usize>;
    unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize;
}

impl<'a, T: Send + Sync> AbstractMut for View<'a, T> {
    type Out = &'a T;
    type Slice = &'a [T];
    unsafe fn get_data(&mut self, index: usize) -> Self::Out {
        &*self.data.as_ptr().add(index)
    }
    unsafe fn get_data_slice(&mut self, indices: std::ops::Range<usize>) -> Self::Slice {
        &std::slice::from_raw_parts(
            self.data.as_ptr().add(indices.start),
            indices.end - indices.start,
        )
    }
    fn indices(&self) -> *const EntityId {
        self.dense.as_ptr()
    }
    unsafe fn mark_modified(&mut self, index: usize) -> Self::Out {
        self.get_data(index)
    }
    unsafe fn mark_id_modified(&mut self, entity: EntityId) -> Self::Out {
        self.get_data(self.index_of_unchecked(entity))
    }
    unsafe fn id_at(&self, index: usize) -> EntityId {
        *self.dense.get_unchecked(index)
    }
    fn index_of(&self, entity: EntityId) -> Option<usize> {
        if self.contains(entity) {
            Some(unsafe { *self.sparse.get_unchecked(entity.index()) })
        } else {
            None
        }
    }
    unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
        *self.sparse.get_unchecked(entity.index())
    }
}

impl<'a, T: Send + Sync> AbstractMut for &View<'a, T> {
    type Out = &'a T;
    type Slice = &'a [T];
    unsafe fn get_data(&mut self, index: usize) -> Self::Out {
        &*self.data.as_ptr().add(index)
    }
    unsafe fn get_data_slice(&mut self, indices: std::ops::Range<usize>) -> Self::Slice {
        std::slice::from_raw_parts(
            self.data.as_ptr().add(indices.start),
            indices.end - indices.start,
        )
    }
    fn indices(&self) -> *const EntityId {
        self.dense.as_ptr()
    }
    unsafe fn mark_modified(&mut self, index: usize) -> Self::Out {
        self.get_data(index)
    }
    unsafe fn mark_id_modified(&mut self, entity: EntityId) -> Self::Out {
        self.get_data(self.index_of_unchecked(entity))
    }
    unsafe fn id_at(&self, index: usize) -> EntityId {
        *self.dense.get_unchecked(index)
    }
    fn index_of(&self, entity: EntityId) -> Option<usize> {
        if self.contains(entity) {
            Some(unsafe { *self.sparse.get_unchecked(entity.index()) })
        } else {
            None
        }
    }
    unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
        *self.sparse.get_unchecked(entity.index())
    }
}

impl<'a, T: 'a + Send + Sync> AbstractMut for RawViewMut<'a, T> {
    type Out = &'a mut T;
    type Slice = &'a mut [T];
    unsafe fn get_data(&mut self, index: usize) -> Self::Out {
        &mut *self.data.add(index)
    }
    unsafe fn get_data_slice(&mut self, indices: std::ops::Range<usize>) -> Self::Slice {
        std::slice::from_raw_parts_mut(self.data.add(indices.start), indices.end - indices.start)
    }
    fn indices(&self) -> *const EntityId {
        self.dense
    }
    unsafe fn mark_modified(&mut self, index: usize) -> Self::Out {
        match &mut (*self.pack_info).pack {
            Pack::Update(pack) => {
                // index of the first element non modified
                let non_mod = pack.inserted + pack.modified;
                if index >= non_mod {
                    std::ptr::swap(self.dense.add(non_mod), self.dense.add(index));
                    std::ptr::swap(self.data.add(non_mod), self.data.add(index));
                    *self.sparse.add((*self.dense.add(non_mod)).index()) = non_mod;
                    *self.sparse.add((*self.dense.add(index)).index()) = index;
                    pack.modified += 1;
                    &mut *self.data.add(non_mod)
                } else {
                    self.get_data(index)
                }
            }
            _ => self.get_data(index),
        }
    }
    unsafe fn mark_id_modified(&mut self, entity: EntityId) -> Self::Out {
        let index = self.index_of_unchecked(entity);
        match &mut (*self.pack_info).pack {
            Pack::Update(pack) => {
                // index of the first element non modified
                let non_mod = pack.inserted + pack.modified;
                if index >= non_mod {
                    std::ptr::swap(self.dense.add(non_mod), self.dense.add(index));
                    std::ptr::swap(self.data.add(non_mod), self.data.add(index));
                    *self.sparse.add((*self.dense.add(non_mod)).index()) = non_mod;
                    *self.sparse.add((*self.dense.add(index)).index()) = index;
                    pack.modified += 1;
                    &mut *self.data.add(non_mod)
                } else {
                    self.get_data(index)
                }
            }
            _ => self.get_data(index),
        }
    }
    unsafe fn id_at(&self, index: usize) -> EntityId {
        *self.dense.add(index)
    }
    fn index_of(&self, entity: EntityId) -> Option<usize> {
        unsafe {
            if self.contains(entity) {
                Some(*self.sparse.add(entity.index()))
            } else {
                None
            }
        }
    }
    unsafe fn index_of_unchecked(&self, entity: EntityId) -> usize {
        *self.sparse.add(entity.index())
    }
}

impl<'a, T: Send + Sync> AbstractMut for Not<View<'a, T>> {
    type Out = ();
    type Slice = ();
    unsafe fn get_data(&mut self, index: usize) -> Self::Out {
        if index != std::usize::MAX {
            unreachable!()
        }
    }
    unsafe fn get_data_slice(&mut self, _: std::ops::Range<usize>) -> Self::Slice {
        unreachable!()
    }
    fn indices(&self) -> *const EntityId {
        unreachable!()
    }
    unsafe fn mark_modified(&mut self, index: usize) -> Self::Out {
        self.get_data(index)
    }
    unsafe fn mark_id_modified(&mut self, _: EntityId) {}
    unsafe fn id_at(&self, index: usize) -> EntityId {
        *self.0.dense.get_unchecked(index)
    }
    fn index_of(&self, entity: EntityId) -> Option<usize> {
        if self.0.contains(entity) {
            None
        } else {
            Some(std::usize::MAX)
        }
    }
    unsafe fn index_of_unchecked(&self, _: EntityId) -> usize {
        std::usize::MAX
    }
}

impl<'a, T: Send + Sync> AbstractMut for &Not<View<'a, T>> {
    type Out = ();
    type Slice = ();
    unsafe fn get_data(&mut self, index: usize) -> Self::Out {
        if index != std::usize::MAX {
            unreachable!()
        }
    }
    unsafe fn get_data_slice(&mut self, _: std::ops::Range<usize>) -> Self::Slice {
        unreachable!()
    }
    fn indices(&self) -> *const EntityId {
        unreachable!()
    }
    unsafe fn mark_modified(&mut self, index: usize) -> Self::Out {
        self.get_data(index)
    }
    unsafe fn mark_id_modified(&mut self, _: EntityId) {}
    unsafe fn id_at(&self, index: usize) -> EntityId {
        *self.0.dense.get_unchecked(index)
    }
    fn index_of(&self, entity: EntityId) -> Option<usize> {
        if self.0.contains(entity) {
            None
        } else {
            Some(std::usize::MAX)
        }
    }
    unsafe fn index_of_unchecked(&self, _: EntityId) -> usize {
        std::usize::MAX
    }
}

impl<'a, T: Send + Sync> AbstractMut for Not<&View<'a, T>> {
    type Out = ();
    type Slice = ();
    unsafe fn get_data(&mut self, index: usize) -> Self::Out {
        if index != std::usize::MAX {
            unreachable!()
        }
    }
    unsafe fn get_data_slice(&mut self, _: std::ops::Range<usize>) -> Self::Slice {
        unreachable!()
    }
    fn indices(&self) -> *const EntityId {
        unreachable!()
    }
    unsafe fn mark_modified(&mut self, index: usize) -> Self::Out {
        self.get_data(index)
    }
    unsafe fn mark_id_modified(&mut self, _: EntityId) {}
    unsafe fn id_at(&self, index: usize) -> EntityId {
        *self.0.dense.get_unchecked(index)
    }
    fn index_of(&self, entity: EntityId) -> Option<usize> {
        if self.0.contains(entity) {
            None
        } else {
            Some(std::usize::MAX)
        }
    }
    unsafe fn index_of_unchecked(&self, _: EntityId) -> usize {
        std::usize::MAX
    }
}

impl<'a, T: Send + Sync> AbstractMut for Not<RawViewMut<'a, T>> {
    type Out = ();
    type Slice = ();
    unsafe fn get_data(&mut self, index: usize) -> Self::Out {
        if index != std::usize::MAX {
            unreachable!()
        }
    }
    unsafe fn get_data_slice(&mut self, _: std::ops::Range<usize>) -> Self::Slice {
        unreachable!()
    }
    fn indices(&self) -> *const EntityId {
        unreachable!()
    }
    unsafe fn mark_modified(&mut self, index: usize) -> Self::Out {
        self.get_data(index)
    }
    unsafe fn mark_id_modified(&mut self, _: EntityId) {}
    unsafe fn id_at(&self, index: usize) -> EntityId {
        *self.0.dense.add(index)
    }
    fn index_of(&self, entity: EntityId) -> Option<usize> {
        if unsafe { self.0.contains(entity) } {
            None
        } else {
            Some(std::usize::MAX)
        }
    }
    unsafe fn index_of_unchecked(&self, _: EntityId) -> usize {
        std::usize::MAX
    }
}
