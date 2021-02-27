use crate::view::{View, ViewMut};
use core::ops::Not as NotOps;

/// Used to filter out components.
///
/// Get and iterators will skip entities that have this component.
///
/// ### Example
/// ```
/// use shipyard::{IntoIter, View, World};
///
/// let mut world = World::new();
///
/// world.add_entity((0usize, 1u32));
/// world.add_entity((2usize,));
///
/// let (usizes, u32s) = world.borrow::<(View<usize>, View<u32>)>().unwrap();
///
/// let mut iter = (&usizes, !&u32s).iter();
/// assert_eq!(iter.next(), Some((&2, ())));
/// assert_eq!(iter.next(), None);
/// let mut iter = (&usizes, &u32s).iter();
/// assert_eq!(iter.next(), Some((&0, &1)));
/// assert_eq!(iter.next(), None);
/// ```
#[derive(Copy, Clone)]
pub struct Not<T>(pub(crate) T);

impl<T> NotOps for &View<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for &ViewMut<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for &mut ViewMut<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}
