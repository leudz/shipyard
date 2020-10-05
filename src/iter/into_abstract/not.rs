use super::IntoAbstract;
use crate::not::Not;
use crate::sparse_set::{FullRawWindowMut, Metadata, SparseSet};
use crate::storage::EntityId;
use crate::type_id::TypeId;
use crate::view::{View, ViewMut};

impl<'a: 'b, 'b, T: 'static> IntoAbstract for Not<&'b View<'a, T>> {
    type AbsView = Not<&'b SparseSet<T>>;
    type Pack = T;

    fn into_abstract(self) -> Self::AbsView {
        Not(&self.0)
    }
    fn len(&self) -> Option<(usize, bool)> {
        None
    }
    fn metadata(&self) -> &Metadata<Self::Pack> {
        &self.0.metadata
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<Not<T>>()
    }
    fn dense(&self) -> *const EntityId {
        unreachable!()
    }
}

impl<'a: 'b, 'b, T: 'static> IntoAbstract for Not<&'b ViewMut<'a, T>> {
    type AbsView = Not<&'b SparseSet<T>>;
    type Pack = T;

    fn into_abstract(self) -> Self::AbsView {
        Not(&self.0)
    }
    fn len(&self) -> Option<(usize, bool)> {
        None
    }
    fn metadata(&self) -> &Metadata<Self::Pack> {
        &self.0.metadata
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<Not<T>>()
    }
    fn dense(&self) -> *const EntityId {
        unreachable!()
    }
}

impl<'a: 'b, 'b, T: 'static> IntoAbstract for Not<&'b mut ViewMut<'a, T>> {
    type AbsView = Not<FullRawWindowMut<'b, T>>;
    type Pack = T;

    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.full_raw_window_mut())
    }
    fn len(&self) -> Option<(usize, bool)> {
        None
    }
    fn metadata(&self) -> &Metadata<Self::Pack> {
        &self.0.metadata
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<Not<T>>()
    }
    fn dense(&self) -> *const EntityId {
        unreachable!()
    }
}
