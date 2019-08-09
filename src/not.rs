use crate::sparse_array::{Read, View, ViewMut, Write};
use std::ops::Not as NotOps;

/// Used to filter out components.
/// Get and iterators will skip entities that have this component.
/// # Example
/// ```
/// # use shipyard::*;
/// let world = World::new::<(usize, u32)>();
/// world.new_entity((0usize, 1u32));
/// world.new_entity((2usize,));
///
/// world.run::<(&usize, Not<&u32>), _>(|(usizes, not_u32s)| {
///     let mut iter = (&usizes, &not_u32s).iter();
///     assert_eq!(iter.next(), Some((&2, ())));
///     assert_eq!(iter.next(), None);
///
///     // the storage is back to normal
///     let u32s = not_u32s.into_inner();
///
///     let mut iter = (&usizes, &u32s).iter();
///     assert_eq!(iter.next(), Some((&0, &1)));
///     assert_eq!(iter.next(), None);
///
///     // we reverse it again, just for this iteration
///     let mut iter = (&usizes, !&u32s).iter();
///     assert_eq!(iter.next(), Some((&2, ())));
///     assert_eq!(iter.next(), None);
/// });
/// ```
#[derive(Copy, Clone)]
pub struct Not<T>(pub(crate) T);

impl<T> Not<T> {
    /// Returns the usual `T` storage.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> NotOps for Read<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for &Read<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for Write<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for &Write<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for &mut Write<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for View<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for &View<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for ViewMut<'_, T> {
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
