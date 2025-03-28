use crate::component::Component;
use crate::views::{View, ViewMut};

/// Allows iteration over a component that may be absent.
///
/// ### Example:
///
/// ```
/// use shipyard::{Component, IntoIter, View, World};
///
/// #[derive(Component, PartialEq, Eq, Debug)]
/// struct A(u32);
///
/// #[derive(Component, PartialEq, Eq, Debug)]
/// struct B(u32);
///
/// let mut world = World::new();
///
/// world.add_entity((A(0),));
/// world.add_entity((A(1), B(10)));
///
/// let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();
/// let mut iter = (&a, b.as_optional()).iter();
///
/// assert_eq!(iter.next(), Some((&A(0), None)));
/// assert_eq!(iter.next(), Some((&A(1), Some(&B(10)))));
/// ```
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
