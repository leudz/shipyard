//! Iterators types and traits.

mod abstract_mut;
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

pub use abstract_mut::AbstractMut;
pub use into_abstract::IntoAbstract;
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
pub use with_id::{IntoWithId, LastId, WithId};
