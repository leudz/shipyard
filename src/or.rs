use crate::{
    component::Component,
    tracking::{Inserted, Tracking},
    views::{View, ViewMut},
};
use core::ops::BitOr;

/// Yields the entities that have at least one of two components.
///
/// # Example
///
/// ```rust
/// use shipyard::{track, iter::OneOfTwo, Component, IntoIter, View, ViewMut, World};
///
/// #[derive(Component, PartialEq, Eq, Debug)]
/// struct A(u32);
///
/// #[derive(Component, PartialEq, Eq, Debug)]
/// struct B(u32);
///
/// let mut world = World::new();
///
/// world.track_all::<(A, B)>();
///
/// world.add_entity((A(0),));
/// world.add_entity((A(1), B(10)));
/// world.borrow::<ViewMut<A, track::All>>().unwrap().clear_all_inserted();
/// world.add_entity((B(20),));
/// world.add_entity((A(3),));
///
/// let (a, b) = world.borrow::<(View<A, track::All>, View<B, track::All>)>().unwrap();
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

impl<'a, T: Component, Track: Tracking, U> BitOr<U> for &'a View<'a, T, Track> {
    type Output = Or<(Self, U)>;

    fn bitor(self, rhs: U) -> Self::Output {
        Or((self, rhs))
    }
}

impl<'a, T: Component, Track: Tracking, U> BitOr<U> for Inserted<&'a View<'a, T, Track>> {
    type Output = Or<(Self, U)>;

    fn bitor(self, rhs: U) -> Self::Output {
        Or((self, rhs))
    }
}

impl<'a, T: Component, Track: Tracking, U> BitOr<U> for &'a ViewMut<'a, T, Track> {
    type Output = Or<(Self, U)>;

    fn bitor(self, rhs: U) -> Self::Output {
        Or((self, rhs))
    }
}

impl<'a, T: Component, Track: Tracking, U> BitOr<U> for &'a mut ViewMut<'a, T, Track> {
    type Output = Or<(Self, U)>;

    fn bitor(self, rhs: U) -> Self::Output {
        Or((self, rhs))
    }
}

/// Returned when iterating with [`Or`](crate::iter::Or) filter.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
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

pub struct OrWindow<T> {
    pub(crate) storages: T,
    pub(crate) is_captain: bool,
    pub(crate) is_past_first_storage: bool,
}
