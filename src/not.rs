use crate::views::{View, ViewMut};
use std::ops::Not as NotOps;

/// Used to filter out components.
/// Get and iterators will skip entities that have this component.
/// # Example
/// ```
/// # use shipyard::prelude::*;
/// let world = World::new();
///
/// world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(|(mut entities, mut usizes, mut u32s)| {
///     entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
///     entities.add_entity((&mut usizes,), (2usize,));
/// });
///
/// world.run::<(&usize, &u32), _, _>(|(usizes, u32s)| {
///     let mut iter = (&usizes, !&u32s).iter();
///     assert_eq!(iter.next(), Some((&2, ())));
///     assert_eq!(iter.next(), None);
///
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
