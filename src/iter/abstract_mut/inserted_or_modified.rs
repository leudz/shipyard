use super::AbstractMut;
use super::Mut;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::{FullRawWindowMut, SparseSet};
use crate::tracking::InsertedOrModified;

impl<'tmp, T: Component> AbstractMut for InsertedOrModified<&'tmp SparseSet<T, T::Tracking>> {
    type Out = &'tmp T;
    type Index = usize;

    #[inline]
    unsafe fn get_data(&self, index: usize) -> Self::Out {
        self.0.get_data(index)
    }
    #[inline]
    unsafe fn get_datas(&self, index: Self::Index) -> Self::Out {
        self.0.get_datas(index)
    }
    #[inline]
    fn indices_of(&self, entity_id: EntityId, _: usize, _: u16) -> Option<Self::Index> {
        if let Some(index) = self.0.index_of(entity_id) {
            let id = unsafe { *self.0.dense.get_unchecked(index) };

            if id.is_inserted() || id.is_modified() {
                Some(index)
            } else {
                None
            }
        } else {
            None
        }
    }
    #[inline]
    unsafe fn indices_of_unchecked(
        &self,
        entity_id: EntityId,
        index: usize,
        mask: u16,
    ) -> Self::Index {
        self.0.indices_of_unchecked(entity_id, index, mask)
    }
    #[inline]
    unsafe fn get_id(&self, index: usize) -> EntityId {
        self.0.get_id(index)
    }
}

impl<'tmp, T: Component> AbstractMut for InsertedOrModified<FullRawWindowMut<'tmp, T>> {
    type Out = Mut<'tmp, T>;
    type Index = usize;

    #[inline]
    unsafe fn get_data(&self, index: usize) -> Self::Out {
        self.0.get_data(index)
    }
    #[inline]
    unsafe fn get_datas(&self, index: Self::Index) -> Self::Out {
        self.0.get_datas(index)
    }
    #[inline]
    fn indices_of(&self, entity_id: EntityId, _: usize, _: u16) -> Option<Self::Index> {
        if let Some(index) = self.0.index_of(entity_id) {
            let id = unsafe { *self.0.dense.add(index) };

            if id.is_inserted() || id.is_modified() {
                Some(index)
            } else {
                None
            }
        } else {
            None
        }
    }
    #[inline]
    unsafe fn indices_of_unchecked(
        &self,
        entity_id: EntityId,
        index: usize,
        mask: u16,
    ) -> Self::Index {
        self.0.indices_of_unchecked(entity_id, index, mask)
    }
    #[inline]
    unsafe fn get_id(&self, index: usize) -> EntityId {
        self.0.get_id(index)
    }
}
