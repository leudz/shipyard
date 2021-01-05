use super::IntoAbstract;
use crate::entity_id::EntityId;
use crate::pack::update::Modified;
use crate::sparse_set::{FullRawWindowMut, Metadata, SparseSet};
use crate::type_id::TypeId;
use crate::view::{View, ViewMut};

impl<'tmp, 'v, T: 'static> IntoAbstract for Modified<&'tmp View<'v, T>> {
    type AbsView = Modified<&'tmp SparseSet<T>>;
    type Pack = T;

    fn into_abstract(self) -> Self::AbsView {
        Modified(&self.0)
    }
    fn len(&self) -> Option<(usize, bool)> {
        Some(((**self.0).len(), false))
    }
    fn metadata(&self) -> &Metadata<Self::Pack> {
        &self.0.metadata
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }
    fn dense(&self) -> *const EntityId {
        self.0.dense.as_ptr()
    }
}

impl<'a: 'b, 'b, T: 'static> IntoAbstract for Modified<&'b ViewMut<'a, T>> {
    type AbsView = Modified<&'b SparseSet<T>>;
    type Pack = T;

    fn into_abstract(self) -> Self::AbsView {
        Modified(&self.0)
    }
    fn len(&self) -> Option<(usize, bool)> {
        Some(((*self.0).len(), false))
    }
    fn metadata(&self) -> &Metadata<Self::Pack> {
        &self.0.metadata
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }
    fn dense(&self) -> *const EntityId {
        self.0.dense.as_ptr()
    }
}

impl<'a: 'b, 'b, T: 'static> IntoAbstract for Modified<&'b mut ViewMut<'a, T>> {
    type AbsView = Modified<FullRawWindowMut<'b, T>>;
    type Pack = T;

    fn into_abstract(self) -> Self::AbsView {
        Modified(self.0.full_raw_window_mut())
    }
    fn len(&self) -> Option<(usize, bool)> {
        Some(((*self.0).len(), false))
    }
    fn metadata(&self) -> &Metadata<Self::Pack> {
        &self.0.metadata
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }
    fn dense(&self) -> *const EntityId {
        self.0.dense.as_ptr()
    }
}
