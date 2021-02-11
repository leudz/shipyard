//! Types used to sort storages.

mod unstable;

use super::SparseSet;
pub use unstable::*;

/// Trait used to sort storage(s).
pub trait IntoSortable {
    /// Type of the sort helper struct.
    type IntoSortable;

    /// Doesn't sort the view(s) until an algorithm is chosen, `unstable` for example.  
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{EntitiesViewMut, IntoSortable, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world.run(|mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>| {
    ///     entities.add_entity(&mut usizes, 1);
    ///     entities.add_entity(&mut usizes, 0);
    ///     usizes.sort().unstable(Ord::cmp);
    /// });
    /// ```
    fn sort(self) -> Self::IntoSortable;
}
