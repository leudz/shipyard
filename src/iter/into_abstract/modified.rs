use super::IntoAbstract;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::{FullRawWindowMut, SparseSet};
use crate::tracking::Modified;
use crate::type_id::TypeId;
use crate::view::{View, ViewMut};

impl<'tmp, 'v, T: Component> IntoAbstract for Modified<&'tmp View<'v, T>> {
    type AbsView = Modified<&'tmp SparseSet<T, T::Tracking>>;
    type Pack = T;

    fn into_abstract(self) -> Self::AbsView {
        Modified(&self.0)
    }
    fn len(&self) -> Option<(usize, bool)> {
        Some(((**self.0).len(), false))
    }
    fn is_tracking_insertion(&self) -> bool {
        self.0.is_tracking_insertion()
    }
    fn is_tracking_modification(&self) -> bool {
        self.0.is_tracking_modification()
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T, T::Tracking>>()
    }
    #[inline]
    fn inner_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn dense(&self) -> *const EntityId {
        self.0.dense.as_ptr()
    }
}

impl<'a: 'b, 'b, T: Component> IntoAbstract for Modified<&'b ViewMut<'a, T>> {
    type AbsView = Modified<&'b SparseSet<T, T::Tracking>>;
    type Pack = T;

    fn into_abstract(self) -> Self::AbsView {
        Modified(&self.0)
    }
    fn len(&self) -> Option<(usize, bool)> {
        Some(((*self.0).len(), false))
    }
    fn is_tracking_insertion(&self) -> bool {
        self.0.is_tracking_insertion()
    }
    fn is_tracking_modification(&self) -> bool {
        self.0.is_tracking_modification()
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T, T::Tracking>>()
    }
    #[inline]
    fn inner_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn dense(&self) -> *const EntityId {
        self.0.dense.as_ptr()
    }
}

impl<'a: 'b, 'b, T: Component> IntoAbstract for Modified<&'b mut ViewMut<'a, T>> {
    type AbsView = Modified<FullRawWindowMut<'b, T>>;
    type Pack = T;

    fn into_abstract(self) -> Self::AbsView {
        Modified(self.0.full_raw_window_mut())
    }
    fn len(&self) -> Option<(usize, bool)> {
        Some(((*self.0).len(), false))
    }
    fn is_tracking_insertion(&self) -> bool {
        self.0.is_tracking_insertion()
    }
    fn is_tracking_modification(&self) -> bool {
        self.0.is_tracking_modification()
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T, T::Tracking>>()
    }
    #[inline]
    fn inner_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn dense(&self) -> *const EntityId {
        self.0.dense.as_ptr()
    }
}
