use crate::component::Component;
use crate::views::{View, ViewMut};

#[allow(missing_docs)]
#[derive(Clone)]
pub struct Optional<T>(pub T);

impl<'v, T: Component> View<'v, T> {
    #[allow(missing_docs)]
    pub fn as_optional(&self) -> Optional<&'_ View<'v, T>> {
        Optional(self)
    }
}

impl<'v, T: Component, Track> ViewMut<'v, T, Track> {
    #[allow(missing_docs)]
    pub fn as_optional(&mut self) -> Optional<&'_ mut ViewMut<'v, T, Track>> {
        Optional(self)
    }
}
