use super::IntoAbstract;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut, SparseSet};
use crate::tracking::{Inserted, InsertionTracking};
use crate::type_id::TypeId;
use crate::views::{View, ViewMut};

impl<'tmp, 'v, T: Component, Track> IntoAbstract for Inserted<&'tmp View<'v, T, Track>>
where
    Track: InsertionTracking,
{
    type AbsView = Inserted<FullRawWindow<'tmp, T>>;

    fn into_abstract(self) -> Self::AbsView {
        Inserted(self.0.into_abstract())
    }
    fn len(&self) -> Option<usize> {
        Some((**self.0).len())
    }
    fn is_tracking(&self) -> bool {
        true
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }
    #[inline]
    fn inner_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn dense(&self) -> *const EntityId {
        self.0.dense.as_ptr()
    }
}

impl<'a: 'b, 'b, T: Component, Track> IntoAbstract for Inserted<&'b ViewMut<'a, T, Track>>
where
    Track: InsertionTracking,
{
    type AbsView = Inserted<FullRawWindow<'b, T>>;

    fn into_abstract(self) -> Self::AbsView {
        Inserted(self.0.into_abstract())
    }
    fn len(&self) -> Option<usize> {
        Some((*self.0).len())
    }
    fn is_tracking(&self) -> bool {
        true
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }
    #[inline]
    fn inner_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn dense(&self) -> *const EntityId {
        self.0.dense.as_ptr()
    }
}

impl<'a: 'b, 'b, T: Component, Track> IntoAbstract for Inserted<&'b mut ViewMut<'a, T, Track>>
where
    Track: InsertionTracking,
{
    type AbsView = Inserted<FullRawWindowMut<'b, T, Track>>;

    fn into_abstract(self) -> Self::AbsView {
        Inserted(self.0.into_abstract())
    }
    fn len(&self) -> Option<usize> {
        Some((*self.0).len())
    }
    fn is_tracking(&self) -> bool {
        true
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T>>()
    }
    #[inline]
    fn inner_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn dense(&self) -> *const EntityId {
        self.0.dense.as_ptr()
    }
}
