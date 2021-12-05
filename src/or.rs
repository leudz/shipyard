use crate::{
    component::Component,
    iter::IntoAbstract,
    view::{View, ViewMut},
    Inserted, InsertedOrModified, Modified,
};
use core::ops::BitOr;

/// Yield the entities that have a component or another.
///
/// # Example
///
/// ```rust
/// use shipyard::{Component, IntoIter, OneOfTwo, View, ViewMut, World};
///
/// #[derive(Component, PartialEq, Eq, Debug)]
/// #[track(All)]
/// struct A(u32);
///
/// #[derive(Component, PartialEq, Eq, Debug)]
/// #[track(All)]
/// struct B(u32);
///
/// let mut world = World::new();
///
/// world.add_entity((A(0),));
/// world.add_entity((A(1), B(10)));
/// world.borrow::<ViewMut<A>>().unwrap().clear_all_inserted();
/// world.add_entity((B(20),));
/// world.add_entity((A(3),));
///
/// let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();
///
/// assert_eq!(
///     (a.inserted() | b.inserted()).iter().collect::<Vec<_>>(),
///     vec![
///         OneOfTwo::One(&A(3)),
///         OneOfTwo::Two(&B(10)),
///         OneOfTwo::Two(&B(20))
///     ]
/// );
/// ```
#[derive(Copy, Clone)]
pub struct Or<T>(pub(crate) T);

impl<'a, T: Component, U: IntoAbstract> BitOr<U> for &'a View<'a, T> {
    type Output = Or<(Self, U)>;

    fn bitor(self, rhs: U) -> Self::Output {
        Or((self, rhs))
    }
}

impl<'a, T: Component, U: IntoAbstract> BitOr<U> for Inserted<&'a View<'a, T>> {
    type Output = Or<(Self, U)>;

    fn bitor(self, rhs: U) -> Self::Output {
        Or((self, rhs))
    }
}

impl<'a, T: Component, U: IntoAbstract> BitOr<U> for Modified<&'a View<'a, T>> {
    type Output = Or<(Self, U)>;

    fn bitor(self, rhs: U) -> Self::Output {
        Or((self, rhs))
    }
}

impl<'a, T: Component, U: IntoAbstract> BitOr<U> for InsertedOrModified<&'a View<'a, T>> {
    type Output = Or<(Self, U)>;

    fn bitor(self, rhs: U) -> Self::Output {
        Or((self, rhs))
    }
}

impl<'a, T: Component, U: IntoAbstract> BitOr<U> for &'a ViewMut<'a, T> {
    type Output = Or<(Self, U)>;

    fn bitor(self, rhs: U) -> Self::Output {
        Or((self, rhs))
    }
}

impl<'a, T: Component, U: IntoAbstract> BitOr<U> for &'a mut ViewMut<'a, T> {
    type Output = Or<(Self, U)>;

    fn bitor(self, rhs: U) -> Self::Output {
        Or((self, rhs))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[allow(missing_docs)]
pub enum OneOfTwo<T, U> {
    #[allow(missing_docs)]
    One(T),
    #[allow(missing_docs)]
    Two(U),
}

impl From<usize> for OneOfTwo<usize, usize> {
    fn from(_: usize) -> Self {
        unreachable!()
    }
}
