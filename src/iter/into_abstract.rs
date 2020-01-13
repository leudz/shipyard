use super::abstract_mut::AbstractMut;
use crate::not::Not;
use crate::sparse_set::{Pack, PackInfo, Window, WindowMut};
use crate::views::{View, ViewMut};
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

impl<'a, T: 'static> IntoAbstract for &'a View<'_, T> {
    type AbsView = &'a Window<'a, T>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        &**self
    }
    fn len(&self) -> Option<usize> {
        Some((**self).len())
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

impl<'a, T: 'static> IntoAbstract for Window<'a, T> {
    type AbsView = Window<'a, T>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn len(&self) -> Option<usize> {
        Some((*self).len())
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

impl<'a, T: 'static> IntoAbstract for &'a Window<'a, T> {
    type AbsView = &'a Window<'a, T>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn len(&self) -> Option<usize> {
        Some((**self).len())
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

impl<'a: 'b, 'b, T: 'static> IntoAbstract for &'b ViewMut<'a, T> {
    type AbsView = Window<'b, T>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self.window()
    }
    fn len(&self) -> Option<usize> {
        Some((**self).len())
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

impl<'a: 'b, 'b, T: 'static> IntoAbstract for &'b mut ViewMut<'a, T> {
    type AbsView = WindowMut<'b, T>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self.window_mut()
    }
    fn len(&self) -> Option<usize> {
        Some((**self).len())
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

impl<'a, T: 'static> IntoAbstract for WindowMut<'a, T> {
    type AbsView = WindowMut<'a, T>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn len(&self) -> Option<usize> {
        Some((*self).len())
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

impl<'a: 'b, 'b, T: 'static> IntoAbstract for &'b WindowMut<'a, T> {
    type AbsView = Window<'b, T>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self.as_non_mut()
    }
    fn len(&self) -> Option<usize> {
        Some((**self).len())
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

impl<'a: 'b, 'b, T: 'static> IntoAbstract for &'b mut WindowMut<'a, T> {
    type AbsView = &'b mut WindowMut<'a, T>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        self
    }
    fn len(&self) -> Option<usize> {
        Some((**self).len())
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

impl<'a: 'b, 'b, T: 'static> IntoAbstract for &'b Not<View<'a, T>> {
    type AbsView = Not<&'b Window<'a, T>>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        Not(&*self.0)
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

impl<'a: 'b, 'b, T: 'static> IntoAbstract for Not<&'b View<'a, T>> {
    type AbsView = Not<&'b Window<'a, T>>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        Not(&**self.0)
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

impl<'a: 'b, 'b, T: 'static> IntoAbstract for &'b Not<ViewMut<'a, T>> {
    type AbsView = Not<Window<'b, T>>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.window())
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

impl<'a: 'b, 'b, T: 'static> IntoAbstract for &'b mut Not<ViewMut<'a, T>> {
    type AbsView = Not<WindowMut<'b, T>>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.window_mut())
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

impl<'a: 'b, 'b, T: 'static> IntoAbstract for Not<&'b ViewMut<'a, T>> {
    type AbsView = Not<Window<'b, T>>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.window())
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

impl<'a: 'b, 'b, T: 'static> IntoAbstract for Not<&'b mut ViewMut<'a, T>> {
    type AbsView = Not<WindowMut<'b, T>>;
    type PackType = T;
    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.window_mut())
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
/*
impl<'a, T: 'static> IntoAbstract for RawViewMut<'a, T> {
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
*/
