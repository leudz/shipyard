mod multiple;
#[cfg(feature = "parallel")]
mod par_single;
mod single;

use super::{
    AbstractMut, CurrentId, DoubleEndedShiperator, ExactSizeShiperator, IntoAbstract, Shiperator,
};

#[cfg(feature = "parallel")]
use super::IntoIterator;

pub use multiple::*;
#[cfg(feature = "parallel")]
pub use par_single::ParUpdate1;
pub use single::Update1;
