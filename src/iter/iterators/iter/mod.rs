mod multiple;
#[cfg(feature = "parallel")]
mod par_single;
mod single;

use super::{
    loose::*, non_packed::*, tight::*, update::*, AbstractMut, CurrentId, IntoAbstract, Shiperator,
};

pub use multiple::*;
#[cfg(feature = "parallel")]
pub use par_single::ParIter1;
pub use single::Iter1;
