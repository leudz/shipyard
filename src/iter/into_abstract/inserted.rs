use super::IntoAbstract;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::{FullRawWindowMut, SparseSet};
use crate::track;
use crate::tracking::Inserted;
use crate::type_id::TypeId;
use crate::view::{View, ViewMut};

impl<'tmp, 'v, T: Component<Tracking = track::Insertion>> IntoAbstract
    for Inserted<&'tmp View<'v, T, track::Insertion>>
{
    type AbsView = Inserted<&'tmp SparseSet<T, track::Insertion>>;

    fn into_abstract(self) -> Self::AbsView {
        Inserted(self.0)
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
    for Inserted<&'tmp View<'v, T, track::All>>
{
    type AbsView = Inserted<&'tmp SparseSet<T, track::All>>;

    fn into_abstract(self) -> Self::AbsView {
        Inserted(self.0)
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

impl<'a: 'b, 'b, T: Component<Tracking = track::Insertion>> IntoAbstract
    for Inserted<&'b ViewMut<'a, T, track::Insertion>>
{
    type AbsView = Inserted<&'b SparseSet<T, track::Insertion>>;

    fn into_abstract(self) -> Self::AbsView {
        Inserted(self.0)
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
    for Inserted<&'b ViewMut<'a, T, track::All>>
{
    type AbsView = Inserted<&'b SparseSet<T, track::All>>;

    fn into_abstract(self) -> Self::AbsView {
        Inserted(self.0)
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

impl<'a: 'b, 'b, T: Component<Tracking = track::Insertion>> IntoAbstract
    for Inserted<&'b mut ViewMut<'a, T, track::Insertion>>
{
    type AbsView = Inserted<FullRawWindowMut<'b, T, track::Insertion>>;

    fn into_abstract(self) -> Self::AbsView {
        Inserted(self.0.full_raw_window_mut())
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
    for Inserted<&'b mut ViewMut<'a, T, track::All>>
{
    type AbsView = Inserted<FullRawWindowMut<'b, T, track::All>>;

    fn into_abstract(self) -> Self::AbsView {
        Inserted(self.0.full_raw_window_mut())
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
