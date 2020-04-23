mod multiple;
mod single;

use super::abstract_mut::AbstractMut;
use super::into_abstract::IntoAbstract;
use super::iterators::*;

/// Trait used to create iterators.
///
/// `std::iter::IntoIterator` can't be used directly because of conflicting implementation.  
/// This trait serves as substitute.
pub trait IntoIter {
    type IntoIter;
    #[cfg(feature = "parallel")]
    type IntoParIter;
    /// Returns an iterator over storages yielding only components meeting the requirements.
    /// ### Example
    /// ```
    /// use shipyard::{EntitiesViewMut, IntoIter, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world.run(
    ///     |mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///         entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///         entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    ///
    ///         for (x, &y) in (&mut usizes, &u32s).iter() {
    ///             *x += y as usize;
    ///         }
    ///     },
    /// );
    /// ```
    /// [run]: ../struct.World.html#method.run
    fn iter(self) -> Self::IntoIter;
    /// Returns a parallel iterator over storages yielding only components meeting the requirements.
    ///
    /// Iterators can only be made inside [run] closure and systems.
    /// ### Example
    /// ```
    /// use rayon::prelude::ParallelIterator;
    /// use shipyard::{EntitiesViewMut, IntoIter, ThreadPoolView, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world.run(
    ///     |mut entities: EntitiesViewMut,
    ///      mut usizes: ViewMut<usize>,
    ///      mut u32s: ViewMut<u32>,
    ///      thread_pool: ThreadPoolView| {
    ///         entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///         entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    ///
    ///         thread_pool.install(|| {
    ///             (&mut usizes, &u32s).par_iter().for_each(|(x, &y)| {
    ///                 *x += y as usize;
    ///             });
    ///         })
    ///     },
    /// );
    /// ```
    /// [run]: ../struct.World.html#method.run
    #[cfg(feature = "parallel")]
    fn par_iter(self) -> Self::IntoParIter;
}

/// Shorthand for a Shiperator only yielding ids.
pub trait IntoIterIds {
    type IntoIterIds;
    fn iter_ids(self) -> Self::IntoIterIds;
}
