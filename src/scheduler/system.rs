use super::TypeInfo;
use crate::all_storages::AllStorages;
use crate::error;
use crate::scheduler::info::DedupedLabels;
use crate::scheduler::label::Label;
use crate::scheduler::workload::Workload;
use crate::type_id::TypeId;
use crate::world::World;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Self contained system that may be inserted into a [`Workload`].
///
/// ### Example:
///
/// ```rust
/// use shipyard::{Component, IntoWorkloadSystem, View, Workload, scheduler::WorkloadSystem, World};
///
/// #[derive(Component)]
/// struct U32(u32);
///
/// #[derive(Component)]
/// struct USIZE(usize);
///
/// fn sys1(u32s: View<U32>) {}
/// fn sys2(usizes: View<USIZE>) {}
///
/// let workload_sys1: WorkloadSystem = sys1.into_workload_system().unwrap();
///
/// let mut workload = Workload::new("my_workload")
///     .with_system(workload_sys1)
///     .with_system(sys2);
/// ```
///
/// [`Workload`]: crate::Workload
#[allow(clippy::type_complexity)]
pub struct WorkloadSystem {
    #[allow(missing_docs)]
    pub type_id: TypeId,
    #[allow(missing_docs)]
    pub display_name: Box<dyn Label>,
    #[allow(missing_docs)]
    pub system_fn: Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>,
    /// access information
    pub borrow_constraints: Vec<TypeInfo>,
    /// Generates the tracking to enable for this system's views
    pub tracking_to_enable: Vec<fn(&AllStorages) -> Result<(), error::GetStorage>>,
    /// Generates constraints and system type id
    pub generator: Box<dyn Fn(&mut Vec<TypeInfo>) -> TypeId + Send + Sync + 'static>,
    #[allow(missing_docs)]
    pub run_if: Option<Box<dyn Fn(&World) -> Result<bool, error::Run> + Send + Sync + 'static>>,
    #[allow(missing_docs)]
    pub tags: Vec<Box<dyn Label>>,
    #[allow(missing_docs)]
    pub before_all: DedupedLabels,
    #[allow(missing_docs)]
    pub after_all: DedupedLabels,
    #[allow(missing_docs)]
    pub require_in_workload: DedupedLabels,
    #[allow(missing_docs)]
    pub require_before: DedupedLabels,
    #[allow(missing_docs)]
    pub require_after: DedupedLabels,
}

impl Extend<WorkloadSystem> for Workload {
    fn extend<T: IntoIterator<Item = WorkloadSystem>>(&mut self, iter: T) {
        self.systems.extend(iter);
    }
}

#[allow(clippy::type_complexity)]
pub struct RunIf {
    pub(crate) system_fn: Box<dyn Fn(&World) -> Result<bool, error::Run> + Send + Sync + 'static>,
}

pub trait WorkloadRunIfFn: Send + Sync + 'static {
    fn run(&self, world: &'_ World) -> Result<bool, error::Run>;
    fn clone(&self) -> Box<dyn WorkloadRunIfFn>;
}

impl<F: Fn(&World) -> Result<bool, error::Run> + Clone + Send + Sync + 'static> WorkloadRunIfFn
    for F
{
    fn run(&self, world: &'_ World) -> Result<bool, error::Run> {
        (self)(world)
    }

    fn clone(&self) -> Box<dyn WorkloadRunIfFn> {
        Box::new(self.clone())
    }
}

#[allow(clippy::type_complexity)]
pub(crate) trait ExtractWorkloadRunIf {
    fn to_non_clone(
        self,
    ) -> Box<dyn Fn(&World) -> Result<bool, error::Run> + Send + Sync + 'static>;
}

impl ExtractWorkloadRunIf for Box<dyn WorkloadRunIfFn> {
    fn to_non_clone(
        self,
    ) -> Box<dyn Fn(&World) -> Result<bool, error::Run> + Send + Sync + 'static> {
        Box::new(move |world| self.run(world))
    }
}

impl Clone for Box<dyn WorkloadRunIfFn> {
    fn clone(&self) -> Box<dyn WorkloadRunIfFn> {
        WorkloadRunIfFn::clone(&**self)
    }
}
