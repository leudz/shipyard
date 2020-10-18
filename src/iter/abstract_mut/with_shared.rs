use super::AbstractMut;
use crate::pack::shared::WithShared;
use crate::sparse_set::SparseSet;
use crate::storage::EntityId;

impl<'tmp, T> AbstractMut for WithShared<&'tmp SparseSet<T>> {
    type Out = &'tmp T;
    type Index = usize;

    #[inline]
    unsafe fn get_data(&self, index: usize) -> Self::Out {
        self.0.data.get_unchecked(index)
    }
    #[inline]
    unsafe fn get_datas(&self, index: Self::Index) -> Self::Out {
        self.0.data.get_unchecked(index)
    }
    #[inline]
    fn indices_of(&self, entity_id: EntityId, _: usize, _: u16) -> Option<Self::Index> {
        self.0.index_of(entity_id)
    }
    #[inline]
    unsafe fn indices_of_unchecked(&self, entity_id: EntityId, _: usize, _: u16) -> Self::Index {
        self.0.index_of(entity_id).unwrap()
    }
    #[inline]
    unsafe fn get_id(&self, index: usize) -> EntityId {
        *self.0.dense.get_unchecked(index)
    }
}
