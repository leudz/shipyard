mod inserted;
mod inserted_or_modified;
mod modified;

use super::AbstractMut;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::not::Not;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};

impl<'w, T: Component> AbstractMut for Not<FullRawWindow<'w, T>> {
    type Out = ();
    type Index = usize;

    #[inline]
    unsafe fn get_data(&self, _: usize) -> Self::Out {}
    #[inline]
    unsafe fn get_datas(&self, _: Self::Index) -> Self::Out {}
    #[inline]
    fn indices_of(&self, entity: EntityId, _: usize, _: u16) -> Option<Self::Index> {
        if self.0.index_of(entity).is_some() {
            None
        } else {
            Some(core::usize::MAX)
        }
    }
    #[inline]
    unsafe fn indices_of_unchecked(&self, _: EntityId, _: usize, _: u16) -> Self::Index {
        unreachable!()
    }
    #[inline]
    unsafe fn get_id(&self, _: usize) -> EntityId {
        unreachable!()
    }
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'w, T: Component> AbstractMut for Not<FullRawWindowMut<'w, T>> {
    type Out = ();
    type Index = usize;

    #[inline]
    unsafe fn get_data(&self, _: usize) -> Self::Out {}
    #[inline]
    unsafe fn get_datas(&self, _: Self::Index) -> Self::Out {}
    #[inline]
    fn indices_of(&self, entity: EntityId, _: usize, _: u16) -> Option<Self::Index> {
        if self.0.index_of(entity).is_some() {
            None
        } else {
            Some(core::usize::MAX)
        }
    }
    #[inline]
    unsafe fn indices_of_unchecked(&self, _: EntityId, _: usize, _: u16) -> Self::Index {
        unreachable!()
    }
    #[inline]
    unsafe fn get_id(&self, _: usize) -> EntityId {
        unreachable!()
    }
    #[inline]
    fn len(&self) -> usize {
        self.0.dense_len
    }
}
