mod unstable;

use super::{SparseSet, WindowMut};
pub use unstable::*;

/// Trait used to sort storage(s).
pub trait IntoSortable {
    type IntoSortable;

    /// Doesn't sort the view(s) until an algorithm is chosen, `unstable` for example.  
    ///
    /// ### Example:
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new();
    /// let (mut entities, mut usizes) = world.borrow::<(EntitiesMut, &mut usize)>();
    /// entities.add_entity(&mut usizes, 1);
    /// entities.add_entity(&mut usizes, 0);
    /// usizes.sort().unstable(Ord::cmp);
    /// ```
    fn sort(self) -> Self::IntoSortable;
}
