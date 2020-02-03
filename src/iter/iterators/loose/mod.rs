mod multiple;
#[cfg(feature = "parallel")]
mod par_multiple;

#[cfg(feature = "parallel")]
use super::IntoIterator;
use super::{
    AbstractMut, CurrentId, DoubleEndedShiperator, ExactSizeShiperator, IntoAbstract, Shiperator,
};

pub use multiple::*;
#[cfg(feature = "parallel")]
pub use par_multiple::*;
