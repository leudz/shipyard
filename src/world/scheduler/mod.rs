mod builder;

pub use builder::WorkloadBuilder;

use crate::error;
use crate::World;
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::TypeId;
use core::ops::Range;
use hashbrown::HashMap;

#[allow(clippy::type_complexity)]
pub(crate) struct Scheduler {
    pub(super) systems: Vec<Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>>,
    pub(super) system_names: Vec<&'static str>,
    pub(super) lookup_table: HashMap<TypeId, usize>,
    // a batch lists systems that can run in parallel
    pub(super) batch: Vec<Box<[usize]>>,
    pub(super) workloads: HashMap<Cow<'static, str>, Range<usize>>,
    pub(super) default: Range<usize>,
}

impl Default for Scheduler {
    #[allow(clippy::reversed_empty_ranges)]
    fn default() -> Self {
        Scheduler {
            systems: Vec::new(),
            system_names: Vec::new(),
            lookup_table: HashMap::new(),
            batch: Vec::new(),
            workloads: HashMap::new(),
            default: 0..0,
        }
    }
}
