use super::abstract_mut::AbstractMut;
use crate::not::Not;
use crate::sparse_set::{Pack, PackInfo, RawViewMut, View, ViewMut};
use std::any::TypeId;

// Allows to make ViewMut's sparse and dense fields immutable
// This is necessary to index into them
#[doc(hidden)]
#[allow(clippy::len_without_is_empty)]
pub trait IntoAbstract {
    type AbsView: AbstractMut;
    type PackType;
    fn into_abstract(self) -> Self::AbsView;
    fn len(&self) -> Option<usize>;
    fn pack_info(&self) -> &PackInfo<Self::PackType>;
    fn type_id(&self) -> TypeId;
    fn modified(&self) -> usize;
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for View<'a, T> {
    type AbsView = Self;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn len(&self) -> Option<usize> {
        Some(View::len(&self))
    }
    fn pack_info(&self) -> &PackInfo<Self::PackType> {
        &self.pack_info
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn modified(&self) -> usize {
        std::usize::MAX
    }
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for &View<'a, T> {
    type AbsView = Self;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn len(&self) -> Option<usize> {
        Some(View::len(&self))
    }
    fn pack_info(&self) -> &PackInfo<Self::PackType> {
        &self.pack_info
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn modified(&self) -> usize {
        std::usize::MAX
    }
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for ViewMut<'a, T> {
    type AbsView = RawViewMut<'a, T>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self.into_raw()
    }
    fn len(&self) -> Option<usize> {
        Some(ViewMut::len(&self))
    }
    fn pack_info(&self) -> &PackInfo<Self::PackType> {
        &self.pack_info
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn modified(&self) -> usize {
        match &self.pack_info.pack {
            Pack::Update(pack) => pack.inserted + pack.modified - 1,
            _ => std::usize::MAX,
        }
    }
}

impl<'a: 'b, 'b, T: 'static + Send + Sync> IntoAbstract for &'b ViewMut<'a, T> {
    type AbsView = View<'b, T>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self.as_non_mut()
    }
    fn len(&self) -> Option<usize> {
        Some(ViewMut::len(&self))
    }
    fn pack_info(&self) -> &PackInfo<Self::PackType> {
        &self.pack_info
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn modified(&self) -> usize {
        std::usize::MAX
    }
}

impl<'a: 'b, 'b, T: 'static + Send + Sync> IntoAbstract for &'b mut ViewMut<'a, T> {
    type AbsView = RawViewMut<'b, T>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self.raw()
    }
    fn len(&self) -> Option<usize> {
        Some(ViewMut::len(&self))
    }
    fn pack_info(&self) -> &PackInfo<Self::PackType> {
        &self.pack_info
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn modified(&self) -> usize {
        match &self.pack_info.pack {
            Pack::Update(pack) => pack.inserted + pack.modified - 1,
            _ => std::usize::MAX,
        }
    }
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for Not<View<'a, T>> {
    type AbsView = Self;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn len(&self) -> Option<usize> {
        None
    }
    fn pack_info(&self) -> &PackInfo<Self::PackType> {
        &self.0.pack_info
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn modified(&self) -> usize {
        std::usize::MAX
    }
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for &Not<View<'a, T>> {
    type AbsView = Self;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn len(&self) -> Option<usize> {
        None
    }
    fn pack_info(&self) -> &PackInfo<Self::PackType> {
        &self.0.pack_info
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn modified(&self) -> usize {
        std::usize::MAX
    }
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for Not<&View<'a, T>> {
    type AbsView = Self;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn len(&self) -> Option<usize> {
        None
    }
    fn pack_info(&self) -> &PackInfo<Self::PackType> {
        &self.0.pack_info
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn modified(&self) -> usize {
        std::usize::MAX
    }
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for Not<ViewMut<'a, T>> {
    type AbsView = Not<RawViewMut<'a, T>>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.into_raw())
    }
    fn len(&self) -> Option<usize> {
        None
    }
    fn pack_info(&self) -> &PackInfo<Self::PackType> {
        &self.0.pack_info
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn modified(&self) -> usize {
        std::usize::MAX
    }
}

impl<'a: 'b, 'b, T: 'static + Send + Sync> IntoAbstract for &'b Not<ViewMut<'a, T>> {
    type AbsView = Not<View<'b, T>>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.as_non_mut())
    }
    fn len(&self) -> Option<usize> {
        None
    }
    fn pack_info(&self) -> &PackInfo<Self::PackType> {
        &self.0.pack_info
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn modified(&self) -> usize {
        std::usize::MAX
    }
}

impl<'a: 'b, 'b, T: 'static + Send + Sync> IntoAbstract for &'b mut Not<ViewMut<'a, T>> {
    type AbsView = Not<RawViewMut<'b, T>>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.raw())
    }
    fn len(&self) -> Option<usize> {
        None
    }
    fn pack_info(&self) -> &PackInfo<Self::PackType> {
        &self.0.pack_info
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn modified(&self) -> usize {
        std::usize::MAX
    }
}

impl<'a: 'b, 'b, T: 'static + Send + Sync> IntoAbstract for Not<&'b ViewMut<'a, T>> {
    type AbsView = Not<View<'b, T>>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.as_non_mut())
    }
    fn len(&self) -> Option<usize> {
        None
    }
    fn pack_info(&self) -> &PackInfo<Self::PackType> {
        &self.0.pack_info
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn modified(&self) -> usize {
        std::usize::MAX
    }
}

impl<'a: 'b, 'b, T: 'static + Send + Sync> IntoAbstract for Not<&'b mut ViewMut<'a, T>> {
    type AbsView = Not<RawViewMut<'b, T>>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.raw())
    }
    fn len(&self) -> Option<usize> {
        None
    }
    fn pack_info(&self) -> &PackInfo<Self::PackType> {
        &self.0.pack_info
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn modified(&self) -> usize {
        std::usize::MAX
    }
}

impl<'a, T: 'static + Send + Sync> IntoAbstract for RawViewMut<'a, T> {
    type AbsView = RawViewMut<'a, T>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn len(&self) -> Option<usize> {
        Some(self.len)
    }
    fn pack_info(&self) -> &PackInfo<Self::PackType> {
        unsafe { &*self.pack_info }
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
    fn modified(&self) -> usize {
        match unsafe { &(*self.pack_info).pack } {
            Pack::Update(pack) => pack.inserted + pack.modified - 1,
            _ => std::usize::MAX,
        }
    }
}
