use super::AbstractMut;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::r#mut::Mut;
use crate::sparse_set::{FullRawWindowMut, SparseSet};
use crate::track;
use crate::tracking::Inserted;

impl<'tmp, T: Component<Tracking = track::Insertion>> AbstractMut
    for Inserted<&'tmp SparseSet<T, track::Insertion>>
{
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
            if unsafe { (*self.0.dense.get_unchecked(index)).is_inserted() } {
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
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'tmp, T: Component<Tracking = track::All>> AbstractMut
    for Inserted<&'tmp SparseSet<T, track::All>>
{
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
            if unsafe { (*self.0.dense.get_unchecked(index)).is_inserted() } {
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
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'tmp, T: Component<Tracking = track::Insertion>> AbstractMut
    for Inserted<FullRawWindowMut<'tmp, T, track::Insertion>>
{
    type Out = &'tmp mut T;
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
            if unsafe { (*self.0.dense.add(index)).is_inserted() } {
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
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'tmp, T: Component<Tracking = track::All>> AbstractMut
    for Inserted<FullRawWindowMut<'tmp, T, track::All>>
{
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
            if unsafe { (*self.0.dense.add(index)).is_inserted() } {
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
    fn len(&self) -> usize {
        self.0.len()
    }
}
