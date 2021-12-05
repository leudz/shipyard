use super::IntoAbstract;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::{FullRawWindowMut, SparseSet};
use crate::track;
use crate::tracking::Modified;
use crate::type_id::TypeId;
use crate::view::{View, ViewMut};

impl<'tmp, 'v, T: Component<Tracking = track::Modification>> IntoAbstract
    for Modified<&'tmp View<'v, T, track::Modification>>
{
    type AbsView = Modified<&'tmp SparseSet<T, track::Modification>>;

    fn into_abstract(self) -> Self::AbsView {
        Modified(self.0)
    }
    fn len(&self) -> Option<usize> {
        Some((**self.0).len())
    }
    fn is_tracking(&self) -> bool {
        true
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

impl<'tmp, 'v, T: Component<Tracking = track::All>> IntoAbstract
    for Modified<&'tmp View<'v, T, track::All>>
{
    type AbsView = Modified<&'tmp SparseSet<T, track::All>>;

    fn into_abstract(self) -> Self::AbsView {
        Modified(self.0)
    }
    fn len(&self) -> Option<usize> {
        Some((**self.0).len())
    }
    fn is_tracking(&self) -> bool {
        true
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

impl<'a: 'b, 'b, T: Component<Tracking = track::Modification>> IntoAbstract
    for Modified<&'b ViewMut<'a, T, track::Modification>>
{
    type AbsView = Modified<&'b SparseSet<T, track::Modification>>;

    fn into_abstract(self) -> Self::AbsView {
        Modified(self.0)
    }
    fn len(&self) -> Option<usize> {
        Some((*self.0).len())
    }
    fn is_tracking(&self) -> bool {
        true
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

impl<'a: 'b, 'b, T: Component<Tracking = track::All>> IntoAbstract
    for Modified<&'b ViewMut<'a, T, track::All>>
{
    type AbsView = Modified<&'b SparseSet<T, track::All>>;

    fn into_abstract(self) -> Self::AbsView {
        Modified(self.0)
    }
    fn len(&self) -> Option<usize> {
        Some((*self.0).len())
    }
    fn is_tracking(&self) -> bool {
        true
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

impl<'a: 'b, 'b, T: Component<Tracking = track::Modification>> IntoAbstract
    for Modified<&'b mut ViewMut<'a, T, track::Modification>>
{
    type AbsView = Modified<FullRawWindowMut<'b, T, track::Modification>>;

    fn into_abstract(self) -> Self::AbsView {
        Modified(self.0.full_raw_window_mut())
    }
    fn len(&self) -> Option<usize> {
        Some((*self.0).len())
    }
    fn is_tracking(&self) -> bool {
        true
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

impl<'a: 'b, 'b, T: Component<Tracking = track::All>> IntoAbstract
    for Modified<&'b mut ViewMut<'a, T, track::All>>
{
    type AbsView = Modified<FullRawWindowMut<'b, T, track::All>>;

    fn into_abstract(self) -> Self::AbsView {
        Modified(self.0.full_raw_window_mut())
    }
    fn len(&self) -> Option<usize> {
        Some((*self.0).len())
    }
    fn is_tracking(&self) -> bool {
        true
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
