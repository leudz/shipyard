//! Iterators types and traits.

mod abstract_mut;
mod fast;
mod into_abstract;
mod into_iter;
#[allow(clippy::module_inception)]
mod iter;
mod mixed;
#[cfg(feature = "rayon")]
mod par_iter;
#[cfg(feature = "rayon")]
mod par_mixed;
#[cfg(feature = "rayon")]
mod par_tight;
mod tight;
mod with_id;

pub use fast::chunk::FastChunk;
pub use fast::chunk_exact::FastChunkExact;
pub use fast::into_iter::IntoFastIter;
pub use fast::iter::FastIter;
pub use fast::mixed::FastMixed;
#[cfg(feature = "rayon")]
pub use fast::par_iter::FastParIter;
#[cfg(feature = "rayon")]
pub use fast::par_mixed::FastParMixed;
#[cfg(feature = "rayon")]
pub use fast::par_tight::FastParTight;
pub use fast::tight::FastTight;
pub use into_iter::IntoIter;
pub use iter::Iter;
pub use mixed::Mixed;
#[cfg(feature = "rayon")]
pub use par_iter::ParIter;
#[cfg(feature = "rayon")]
pub use par_mixed::ParMixed;
#[cfg(feature = "rayon")]
pub use par_tight::ParTight;
pub use tight::Tight;
pub use with_id::{IntoWithId, LastId, WithId};
