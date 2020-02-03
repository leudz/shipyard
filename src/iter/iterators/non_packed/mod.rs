mod multiple;
#[cfg(feature = "parallel")]
mod par_multiple;

use super::{AbstractMut, CurrentId, IntoAbstract, Shiperator};

pub use multiple::*;
#[cfg(feature = "parallel")]
pub use par_multiple::*;
