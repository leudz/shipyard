mod multiple;
mod single;

use super::abstract_mut::AbstractMut;
use super::into_abstract::IntoAbstract;
use super::iterators::*;

// This trait exists because of conflicting implementations
// when using std::iter::IntoIterator
/// Trait used to create iterators.
///
/// `std::iter::Iterator` can't be used because of conflicting implementation.
/// This trait serves as substitute.
pub trait IntoIter {
    type IntoIter;
    #[cfg(feature = "parallel")]
    type IntoParIter;
    /// Returns an iterator over storages yielding only components meeting the requirements.
    ///
    /// Iterators can only be made inside [run] closure and systems.
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new::<(usize, u32)>();
    /// world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(|(mut entities, mut usizes, mut u32s)| {
    ///     entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///     entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    ///     (&mut usizes, &u32s).iter().for_each(|(x, &y)| {
    ///         *x += y as usize;
    ///     });
    /// });
    /// ```
    /// [run]: ../struct.World.html#method.run
    fn iter(self) -> Self::IntoIter;
    /// Returns a parallel iterator over storages yielding only components meeting the requirements.
    ///
    /// Iterators can only be made inside [run] closure and systems.
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
    /// use rayon::prelude::ParallelIterator;
    ///
    /// let world = World::new::<(usize, u32)>();
    /// world.run::<(EntitiesMut, &mut usize, &mut u32, ThreadPool), _, _>(|(mut entities, mut usizes, mut u32s, thread_pool)| {
    ///     entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///     entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    ///     thread_pool.install(|| {
    ///         (&mut usizes, &u32s).par_iter().for_each(|(mut x, &y)| {
    ///             *x += y as usize;
    ///         });
    ///     })
    /// });
    /// ```
    /// [run]: ../struct.World.html#method.run
    #[cfg(feature = "parallel")]
    fn par_iter(self) -> Self::IntoParIter;
}

pub trait IntoIterIds {
    type IntoIterIds;
    fn iter_ids(self) -> Self::IntoIterIds;
}
