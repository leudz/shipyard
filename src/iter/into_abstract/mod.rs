mod inserted;
mod inserted_or_modified;
mod modified;
mod not;

use super::abstract_mut::AbstractMut;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::{FullRawWindowMut, SparseSet};
use crate::sparse_set::{SparseArray, BUCKET_SIZE};
use crate::track;
use crate::type_id::TypeId;
use crate::view::{View, ViewMut};

// Allows to make ViewMut's sparse and dense fields immutable
// This is necessary to index into them
#[doc(hidden)]
#[allow(clippy::len_without_is_empty)]
pub trait IntoAbstract {
    type AbsView: AbstractMut;
    type Pack;

    fn into_abstract(self) -> Self::AbsView;
    fn len(&self) -> Option<(usize, bool)>;
    fn is_tracking_insertion(&self) -> bool;
    fn is_tracking_modification(&self) -> bool;
    fn type_id(&self) -> TypeId;
    fn inner_type_id(&self) -> TypeId;
    fn dense(&self) -> *const EntityId;
    #[inline]
    fn sparse(&self) -> *const SparseArray<EntityId, BUCKET_SIZE> {
        core::ptr::null()
    }
}

impl<'a, T: Component> IntoAbstract for &'a View<'_, T> {
    type AbsView = &'a SparseSet<T, T::Tracking>;
    type Pack = T;

    #[inline]
    fn into_abstract(self) -> Self::AbsView {
        &self
    }
    #[inline]
    fn len(&self) -> Option<(usize, bool)> {
        Some(((**self).len(), true))
    }
    fn is_tracking_insertion(&self) -> bool {
        SparseSet::is_tracking_insertion(&self)
    }
    fn is_tracking_modification(&self) -> bool {
        SparseSet::is_tracking_modification(&self)
    }
    #[inline]
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T, T::Tracking>>()
    }
    #[inline]
    fn inner_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    #[inline]
    fn dense(&self) -> *const EntityId {
        self.dense.as_ptr()
    }
}

impl<'a: 'b, 'b, T: Component> IntoAbstract for &'b ViewMut<'a, T> {
    type AbsView = &'b SparseSet<T, T::Tracking>;
    type Pack = T;

    #[inline]
    fn into_abstract(self) -> Self::AbsView {
        &self
    }
    #[inline]
    fn len(&self) -> Option<(usize, bool)> {
        Some(((**self).len(), true))
    }
    fn is_tracking_insertion(&self) -> bool {
        SparseSet::is_tracking_insertion(&self)
    }
    fn is_tracking_modification(&self) -> bool {
        SparseSet::is_tracking_modification(&self)
    }
    #[inline]
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T, T::Tracking>>()
    }
    #[inline]
    fn inner_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    #[inline]
    fn dense(&self) -> *const EntityId {
        self.dense.as_ptr()
    }
}

impl<'a: 'b, 'b, T: Component<Tracking = track::Nothing>> IntoAbstract
    for &'b mut ViewMut<'a, T, track::Nothing>
{
    type AbsView = FullRawWindowMut<'b, T, track::Nothing>;
    type Pack = T;

    #[inline]
    fn into_abstract(self) -> Self::AbsView {
        self.full_raw_window_mut()
    }
    #[inline]
    fn len(&self) -> Option<(usize, bool)> {
        Some(((**self).len(), true))
    }
    fn is_tracking_insertion(&self) -> bool {
        SparseSet::is_tracking_insertion(&self)
    }
    fn is_tracking_modification(&self) -> bool {
        SparseSet::is_tracking_modification(&self)
    }
    #[inline]
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T, T::Tracking>>()
    }
    #[inline]
    fn inner_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    #[inline]
    fn dense(&self) -> *const EntityId {
        self.dense.as_ptr()
    }
}

impl<'a: 'b, 'b, T: Component<Tracking = track::Insertion>> IntoAbstract
    for &'b mut ViewMut<'a, T, track::Insertion>
{
    type AbsView = FullRawWindowMut<'b, T, track::Insertion>;
    type Pack = T;

    #[inline]
    fn into_abstract(self) -> Self::AbsView {
        self.full_raw_window_mut()
    }
    #[inline]
    fn len(&self) -> Option<(usize, bool)> {
        Some(((**self).len(), true))
    }
    fn is_tracking_insertion(&self) -> bool {
        SparseSet::is_tracking_insertion(&self)
    }
    fn is_tracking_modification(&self) -> bool {
        SparseSet::is_tracking_modification(&self)
    }
    #[inline]
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T, T::Tracking>>()
    }
    #[inline]
    fn inner_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    #[inline]
    fn dense(&self) -> *const EntityId {
        self.dense.as_ptr()
    }
}

impl<'a: 'b, 'b, T: Component<Tracking = track::Modification>> IntoAbstract
    for &'b mut ViewMut<'a, T, track::Modification>
{
    type AbsView = FullRawWindowMut<'b, T, track::Modification>;
    type Pack = T;

    #[inline]
    fn into_abstract(self) -> Self::AbsView {
        self.full_raw_window_mut()
    }
    #[inline]
    fn len(&self) -> Option<(usize, bool)> {
        Some(((**self).len(), true))
    }
    fn is_tracking_insertion(&self) -> bool {
        SparseSet::is_tracking_insertion(&self)
    }
    fn is_tracking_modification(&self) -> bool {
        SparseSet::is_tracking_modification(&self)
    }
    #[inline]
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T, T::Tracking>>()
    }
    #[inline]
    fn inner_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    #[inline]
    fn dense(&self) -> *const EntityId {
        self.dense.as_ptr()
    }
}

impl<'a: 'b, 'b, T: Component<Tracking = track::Removal>> IntoAbstract
    for &'b mut ViewMut<'a, T, track::Removal>
{
    type AbsView = FullRawWindowMut<'b, T, track::Removal>;
    type Pack = T;

    #[inline]
    fn into_abstract(self) -> Self::AbsView {
        self.full_raw_window_mut()
    }
    #[inline]
    fn len(&self) -> Option<(usize, bool)> {
        Some(((**self).len(), true))
    }
    fn is_tracking_insertion(&self) -> bool {
        SparseSet::is_tracking_insertion(&self)
    }
    fn is_tracking_modification(&self) -> bool {
        SparseSet::is_tracking_modification(&self)
    }
    #[inline]
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T, T::Tracking>>()
    }
    #[inline]
    fn inner_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    #[inline]
    fn dense(&self) -> *const EntityId {
        self.dense.as_ptr()
    }
}

impl<'a: 'b, 'b, T: Component<Tracking = track::All>> IntoAbstract
    for &'b mut ViewMut<'a, T, track::All>
{
    type AbsView = FullRawWindowMut<'b, T, track::All>;
    type Pack = T;

    #[inline]
    fn into_abstract(self) -> Self::AbsView {
        self.full_raw_window_mut()
    }
    #[inline]
    fn len(&self) -> Option<(usize, bool)> {
        Some(((**self).len(), true))
    }
    fn is_tracking_insertion(&self) -> bool {
        SparseSet::is_tracking_insertion(&self)
    }
    fn is_tracking_modification(&self) -> bool {
        SparseSet::is_tracking_modification(&self)
    }
    #[inline]
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T, T::Tracking>>()
    }
    #[inline]
    fn inner_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    #[inline]
    fn dense(&self) -> *const EntityId {
        self.dense.as_ptr()
    }
}
