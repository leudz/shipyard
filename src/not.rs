use crate::component::Component;
use crate::views::{View, ViewMut};
use core::ops::Not as NotOps;

/// Used to filter out components.
///
/// Get and iterators will skip entities that have this component.  
/// Simply add `!` in front of the view reference at iterator creation.
///
/// ### Example
/// ```
/// use shipyard::{Component, IntoIter, View, World};
///
/// #[derive(Component, Debug, PartialEq, Eq)]
/// struct U32(u32);
///
/// #[derive(Component, Debug, PartialEq, Eq)]
/// struct USIZE(usize);
///
/// let mut world = World::new();
///
/// world.add_entity((USIZE(0), U32(1)));
/// world.add_entity((USIZE(2),));
///
/// let (usizes, u32s) = world.borrow::<(View<USIZE>, View<U32>)>().unwrap();
///
/// let mut iter = (&usizes, !&u32s).iter();
/// assert_eq!(iter.next(), Some((&USIZE(2), ())));
/// assert_eq!(iter.next(), None);
/// let mut iter = (&usizes, &u32s).iter();
/// assert_eq!(iter.next(), Some((&USIZE(0), &U32(1))));
/// assert_eq!(iter.next(), None);
/// ```
#[derive(Copy, Clone)]
pub struct Not<T>(pub(crate) T);

impl<T: Component, TRACK> NotOps for &View<'_, T, TRACK> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T: Component, TRACK> NotOps for &ViewMut<'_, T, TRACK> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T: Component, TRACK> NotOps for &mut ViewMut<'_, T, TRACK> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}
