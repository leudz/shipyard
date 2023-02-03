mod inserted;
mod inserted_or_modified;
mod modified;
mod not;
mod or;

use super::abstract_mut::AbstractMut;
use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut, SparseSet};
use crate::sparse_set::{SparseArray, BUCKET_SIZE};
use crate::type_id::TypeId;
use crate::view::{View, ViewMut};
use alloc::vec::Vec;

// Allows to make ViewMut's sparse and dense fields immutable
// This is necessary to index into them
#[allow(missing_docs)]
#[allow(clippy::len_without_is_empty)]
pub trait IntoAbstract {
    type AbsView: AbstractMut;

    #[doc(hidden)]
    fn into_abstract(self) -> Self::AbsView;
    #[doc(hidden)]
    fn len(&self) -> Option<usize>;
    #[doc(hidden)]
    fn type_id(&self) -> TypeId;
    #[doc(hidden)]
    fn inner_type_id(&self) -> TypeId;
    #[doc(hidden)]
    fn dense(&self) -> *const EntityId;
    #[inline]
    #[doc(hidden)]
    fn sparse(&self) -> *const SparseArray<EntityId, BUCKET_SIZE> {
        core::ptr::null()
    }
    #[doc(hidden)]
    fn is_tracking(&self) -> bool {
        false
    }
    #[doc(hidden)]
    fn is_not(&self) -> bool {
        false
    }
    #[doc(hidden)]
    fn is_or(&self) -> bool {
        false
    }
    #[doc(hidden)]
    fn other_dense(&self) -> Vec<core::slice::Iter<'static, EntityId>> {
        Vec::new()
    }
}

impl<'a, T: Component, const TRACK: u32> IntoAbstract for &'a View<'a, T, TRACK> {
    type AbsView = FullRawWindow<'a, T>;

    #[inline]
    fn into_abstract(self) -> Self::AbsView {
        FullRawWindow::from_view(self)
    }
    #[inline]
    fn len(&self) -> Option<usize> {
        Some((**self).len())
    }
    #[inline]
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T>>()
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

impl<'a: 'b, 'b, T: Component, const TRACK: u32> IntoAbstract for &'b ViewMut<'a, T, TRACK> {
    type AbsView = FullRawWindow<'b, T>;

    #[inline]
    fn into_abstract(self) -> Self::AbsView {
        FullRawWindow::from_view_mut(self)
    }
    #[inline]
    fn len(&self) -> Option<usize> {
        Some((**self).len())
    }
    #[inline]
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T>>()
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

impl<'a: 'b, 'b, T: Component, const TRACK: u32> IntoAbstract for &'b mut ViewMut<'a, T, TRACK> {
    type AbsView = FullRawWindowMut<'b, T>;

    #[inline]
    fn into_abstract(self) -> Self::AbsView {
        FullRawWindowMut::new(self)
    }
    #[inline]
    fn len(&self) -> Option<usize> {
        Some((**self).len())
    }
    #[inline]
    fn type_id(&self) -> TypeId {
        TypeId::of::<SparseSet<T>>()
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
