use super::IntoAbstract;
use crate::pack::shared::WithShared;
use crate::sparse_set::{Metadata, SparseArray, SparseSet, BUCKET_SIZE, SHARED_BUCKET_SIZE};
use crate::storage::EntityId;
use crate::type_id::TypeId;
use crate::view::{View, ViewMut};

impl<'a, T: 'static> IntoAbstract for WithShared<&'a View<'_, T>> {
    type AbsView = WithShared<&'a SparseSet<T>>;
    type Pack = T;

    #[inline]
    fn into_abstract(self) -> Self::AbsView {
        WithShared(&self.0)
    }
    #[inline]
    fn len(&self) -> Option<(usize, bool)> {
        Some(((*self.0).len(), false))
    }
    #[inline]
    fn metadata(&self) -> &Metadata<Self::Pack> {
        &self.0.metadata
    }
    #[inline]
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }
    #[inline]
    fn dense(&self) -> *const EntityId {
        self.0.dense.as_ptr()
    }
    #[inline]
    fn shared(&self) -> *const SparseArray<[EntityId; SHARED_BUCKET_SIZE]> {
        &self.0.metadata.shared
    }
    #[inline]
    fn sparse(&self) -> *const SparseArray<[EntityId; BUCKET_SIZE]> {
        &self.0.sparse
    }
}

impl<'a: 'b, 'b, T: 'static> IntoAbstract for WithShared<&'b ViewMut<'a, T>> {
    type AbsView = WithShared<&'b SparseSet<T>>;
    type Pack = T;

    #[inline]
    fn into_abstract(self) -> Self::AbsView {
        WithShared(&self.0)
    }
    #[inline]
    fn len(&self) -> Option<(usize, bool)> {
        Some(((*self.0).len(), false))
    }
    #[inline]
    fn metadata(&self) -> &Metadata<Self::Pack> {
        &self.0.metadata
    }
    #[inline]
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }
    #[inline]
    fn dense(&self) -> *const EntityId {
        self.0.dense.as_ptr()
    }
    #[inline]
    fn shared(&self) -> *const SparseArray<[EntityId; SHARED_BUCKET_SIZE]> {
        &self.0.metadata.shared
    }
    #[inline]
    fn sparse(&self) -> *const SparseArray<[EntityId; BUCKET_SIZE]> {
        &self.0.sparse
    }
}
