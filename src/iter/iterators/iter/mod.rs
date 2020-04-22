mod multiple;
#[cfg(feature = "parallel")]
mod par_multiple;
#[cfg(feature = "parallel")]
mod par_single;
mod single;

use super::{
    loose::*, non_packed::*, tight::*, update::*, AbstractMut, CurrentId, IntoAbstract,
    IntoIterator, Shiperator,
};

pub use multiple::*;
#[cfg(feature = "parallel")]
pub use par_multiple::*;
#[cfg(feature = "parallel")]
pub use par_single::ParIter1;
pub use single::Iter1;
