// WIP
//mod function;
mod regular;

// WIP
//pub(super) use function::IntoWorkloadFn;
pub(super) use regular::IntoWorkload;

use crate::error;
use crate::World;
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::TypeId;
use core::ops::Range;
use hashbrown::HashMap;

#[allow(clippy::type_complexity)]
pub struct Scheduler {
    pub(super) systems:
        Vec<Box<dyn for<'a> Fn(&'a World) -> Result<(), error::GetStorage> + Send + Sync>>,
    pub(super) lookup_table: HashMap<TypeId, usize>,
    // a batch list systems running in parallel
    pub(super) batch: Vec<Box<[usize]>>,
    // first usize is the index where the workload begins
    // the second is the number of batch in it
    pub(super) workloads: HashMap<Cow<'static, str>, Range<usize>>,
    pub(super) default: Range<usize>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler {
            systems: Vec::new(),
            lookup_table: HashMap::new(),
            batch: Vec::new(),
            workloads: HashMap::new(),
            default: 0..0,
        }
    }
}
