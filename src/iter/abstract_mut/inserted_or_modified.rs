use super::AbstractMut;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::tracking::InsertedOrModified;

impl<'tmp, T: Component> AbstractMut for InsertedOrModified<FullRawWindow<'tmp, T>> {
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
        self.0.index_of(entity_id).filter(|&index| {
            unsafe { *self.0.insertion_data.add(index) }
                .is_within(self.0.last_insertion, self.0.current)
                || unsafe { *self.0.modification_data.add(index) }
                    .is_within(self.0.last_modification, self.0.current)
        })
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
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'tmp, T: Component, Track> AbstractMut
    for InsertedOrModified<FullRawWindowMut<'tmp, T, Track>>
{
    type Out = &'tmp mut T;
    type Index = usize;

    #[inline]
    unsafe fn get_data(&self, index: usize) -> Self::Out {
        &mut *self.0.data.add(index)
    }
    #[inline]
    unsafe fn get_datas(&self, index: Self::Index) -> Self::Out {
        &mut *self.0.data.add(index)
    }
    #[inline]
    fn indices_of(&self, entity_id: EntityId, _: usize, _: u16) -> Option<Self::Index> {
        self.0.index_of(entity_id).filter(|&index| {
            unsafe { *self.0.insertion_data.add(index) }
                .is_within(self.0.last_insertion, self.0.current)
                || unsafe { *self.0.modification_data.add(index) }
                    .is_within(self.0.last_modification, self.0.current)
        })
    }
    #[inline]
    unsafe fn indices_of_unchecked(
        &self,
        entity_id: EntityId,
        _index: usize,
        _mask: u16,
    ) -> Self::Index {
        self.0.index_of_unchecked(entity_id)
    }
    #[inline]
    unsafe fn get_id(&self, index: usize) -> EntityId {
        *self.0.dense.add(index)
    }
    #[inline]
    fn len(&self) -> usize {
        self.0.dense_len
    }
}
