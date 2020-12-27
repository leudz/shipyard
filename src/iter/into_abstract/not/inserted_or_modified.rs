use super::IntoAbstract;
use crate::entity_id::EntityId;
use crate::not::Not;
use crate::pack::update::InsertedOrModified;
use crate::sparse_set::{FullRawWindowMut, Metadata, SparseSet};
use crate::type_id::TypeId;
use crate::view::{View, ViewMut};

impl<'tmp, 'v, T: 'static> IntoAbstract for Not<InsertedOrModified<&'tmp View<'v, T>>> {
    type AbsView = Not<InsertedOrModified<&'tmp SparseSet<T>>>;
    type Pack = T;

    fn into_abstract(self) -> Self::AbsView {
        Not(InsertedOrModified(&self.0 .0))
    }
    fn len(&self) -> Option<(usize, bool)> {
        Some(((**self.0 .0).len(), false))
    }
    fn metadata(&self) -> &Metadata<Self::Pack> {
        &self.0 .0.metadata
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }
    fn dense(&self) -> *const EntityId {
        self.0 .0.dense.as_ptr()
    }
}

impl<'a: 'b, 'b, T: 'static> IntoAbstract for Not<InsertedOrModified<&'b ViewMut<'a, T>>> {
    type AbsView = Not<InsertedOrModified<&'b SparseSet<T>>>;
    type Pack = T;

    fn into_abstract(self) -> Self::AbsView {
        Not(InsertedOrModified(&self.0 .0))
    }
    fn len(&self) -> Option<(usize, bool)> {
        Some(((*self.0 .0).len(), false))
    }
    fn metadata(&self) -> &Metadata<Self::Pack> {
        &self.0 .0.metadata
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }
    fn dense(&self) -> *const EntityId {
        self.0 .0.dense.as_ptr()
    }
}

impl<'a: 'b, 'b, T: 'static> IntoAbstract for Not<InsertedOrModified<&'b mut ViewMut<'a, T>>> {
    type AbsView = Not<InsertedOrModified<FullRawWindowMut<'b, T>>>;
    type Pack = T;

    fn into_abstract(self) -> Self::AbsView {
        Not(InsertedOrModified(self.0 .0.full_raw_window_mut()))
    }
    fn len(&self) -> Option<(usize, bool)> {
        Some(((*self.0 .0).len(), false))
    }
    fn metadata(&self) -> &Metadata<Self::Pack> {
        &self.0 .0.metadata
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }
    fn dense(&self) -> *const EntityId {
        self.0 .0.dense.as_ptr()
    }
}
