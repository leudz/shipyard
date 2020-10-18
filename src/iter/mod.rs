mod abstract_mut;
mod fast;
mod into_abstract;
mod into_iter;
#[allow(clippy::module_inception)]
mod iter;
mod mixed;
#[cfg(feature = "parallel")]
mod par_iter;
#[cfg(feature = "parallel")]
mod par_mixed;
#[cfg(feature = "parallel")]
mod par_tight;
mod tight;
mod with_id;

pub use fast::chunk::FastChunk;
pub use fast::chunk_exact::FastChunkExact;
pub use fast::into_iter::IntoFastIter;
pub use fast::iter::FastIter;
pub use fast::mixed::FastMixed;
#[cfg(feature = "parallel")]
pub use fast::par_iter::FastParIter;
#[cfg(feature = "parallel")]
pub use fast::par_mixed::FastParMixed;
#[cfg(feature = "parallel")]
pub use fast::par_tight::FastParTight;
pub use fast::tight::FastTight;
pub use into_iter::IntoIter;
pub use iter::Iter;
pub use mixed::Mixed;
#[cfg(feature = "parallel")]
pub use par_iter::ParIter;
#[cfg(feature = "parallel")]
pub use par_mixed::ParMixed;
#[cfg(feature = "parallel")]
pub use par_tight::ParTight;
pub use tight::Tight;
pub use with_id::{IntoWithId, WithId};
