use super::AbstractMut;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::not::Not;
use crate::r#mut::Mut;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::track;
use crate::tracking::Inserted;

impl<'tmp, T: Component<Tracking = track::Insertion>> AbstractMut
    for Not<Inserted<FullRawWindow<'tmp, T, track::Insertion>>>
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
        (self.0).0.index_of(entity_id).filter(|&index| unsafe {
            !track::is_track_within_bounds(
                *(self.0).0.insertion_data.add(index),
                (self.0).0.last_insertion,
                (self.0).0.current,
            )
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

impl<'tmp, T: Component<Tracking = track::All>> AbstractMut
    for Not<Inserted<FullRawWindow<'tmp, T, track::All>>>
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
        (self.0).0.index_of(entity_id).filter(|&index| unsafe {
            !track::is_track_within_bounds(
                *(self.0).0.insertion_data.add(index),
                (self.0).0.last_insertion,
                (self.0).0.current,
            )
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

impl<'tmp, T: Component<Tracking = track::Insertion>> AbstractMut
    for Not<Inserted<FullRawWindowMut<'tmp, T, track::Insertion>>>
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
        (self.0).0.index_of(entity_id).filter(|&index| unsafe {
            !track::is_track_within_bounds(
                *(self.0).0.insertion_data.add(index),
                (self.0).0.last_insertion,
                (self.0).0.current,
            )
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

impl<'tmp, T: Component<Tracking = track::All>> AbstractMut
    for Not<Inserted<FullRawWindowMut<'tmp, T, track::All>>>
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
        (self.0).0.index_of(entity_id).filter(|&index| unsafe {
            !track::is_track_within_bounds(
                *(self.0).0.insertion_data.add(index),
                (self.0).0.last_insertion,
                (self.0).0.current,
            )
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
