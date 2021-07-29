use super::{TypeInfo, WorkloadBuilder};
use crate::error;
use crate::type_id::TypeId;
use crate::world::World;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Self contained system that may be inserted into a [`WorkloadBuilder`].
///
/// ### Example:
///
/// ```rust
/// use shipyard::{Component, IntoWorkloadSystem, View, Workload, WorkloadSystem, World};
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
/// let mut workload = Workload::builder("my_workload")
///     .with_system(workload_sys1)
///     .with_system(sys2);
/// ```
///
/// [`WorkloadBuilder`]: crate::WorkloadBuilder
pub struct WorkloadSystem {
    pub(super) system_type_id: TypeId,
    pub(super) system_type_name: &'static str,
    pub(super) system_fn: Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>,
    /// access information
    pub(super) borrow_constraints: Vec<TypeInfo>,
    pub(super) generator: fn(&mut Vec<TypeInfo>) -> TypeId,
}

impl Extend<WorkloadSystem> for WorkloadBuilder {
    fn extend<T: IntoIterator<Item = WorkloadSystem>>(&mut self, iter: T) {
        self.systems.extend(iter.into_iter().map(Into::into));
    }
}
