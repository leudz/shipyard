use crate::view::{View, ViewMut};
use core::ops::Not as NotOps;

/// Used to filter out components.
/// Get and iterators will skip entities that have this component.
/// ### Example
/// ```
/// use shipyard::{EntitiesViewMut, IntoIter, Shiperator, View, ViewMut, World};
///
/// let world = World::new();
///
/// world.run(
///     |mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
///         entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
///         entities.add_entity((&mut usizes,), (2usize,));
///     },
/// );
///
/// world.run(|usizes: View<usize>, u32s: View<u32>| {
///     let mut iter = (&usizes, !&u32s).iter();
///     assert_eq!(iter.next(), Some((&2, ())));
///     assert_eq!(iter.next(), None);
///     let mut iter = (&usizes, &u32s).iter();
///     assert_eq!(iter.next(), Some((&0, &1)));
///     assert_eq!(iter.next(), None);
/// });
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
