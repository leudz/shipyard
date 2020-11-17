use super::AbstractMut;
use crate::not::Not;
use crate::sparse_set::{FullRawWindowMut, SparseSet};
use crate::storage::EntityId;

impl<'w, T> AbstractMut for Not<&'w SparseSet<T>> {
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
}

impl<'w, T> AbstractMut for Not<FullRawWindowMut<'w, T>> {
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
}
