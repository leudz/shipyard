use core::marker::PhantomData;

/// Mimics an exclusive borrow of `T` without actually borrowing anything.  
/// Can be useful to correctly schedule `Sync` types.
///
/// Use [`Unique<T>`] for unique storage.
/// ### Example:
/// ```
/// use shipyard::{FakeBorrow, IntoWorkloadSystem, SparseSet, View, Workload, World};
///
/// fn display_first(_: View<usize>) {}
/// fn display_next(_: View<usize>) {}
///
/// let world = World::new();
///
/// Workload::builder("Display")
///     .with_system(display_first.into_workload_system().unwrap())
///     .with_system((|_: FakeBorrow<SparseSet<usize>>| {}).into_workload_system().unwrap())
///     .with_system(display_next.into_workload_system().unwrap())
///     .add_to_world(&world)
///     .unwrap();
/// ```
///
/// [`Unique<T>`]: crate::Unique
pub struct FakeBorrow<T: ?Sized>(PhantomData<T>);

impl<T: ?Sized> FakeBorrow<T> {
    pub(crate) fn new() -> Self {
        FakeBorrow(PhantomData)
    }
}
