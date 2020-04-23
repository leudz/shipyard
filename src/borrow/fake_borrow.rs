use core::marker::PhantomData;

/// Mimics an exclusive borrow of `T` without actually doing it.
///
/// Can be useful to correctly schedule `Sync` types.
/// ### Example:
/// ```
/// use shipyard::{system, FakeBorrow, View, World};
///
/// fn display_first(_: View<usize>) {}
/// fn display_next(_: View<usize>) {}
///
/// let world = World::new();
///
/// world
///     .add_workload("Display")
///     .with_system(system!(display_first))
///     .with_system(system!(|_: FakeBorrow<usize>| {}))
///     .with_system(system!(display_next))
///     .build();
/// ```
pub struct FakeBorrow<T: ?Sized>(PhantomData<T>);

impl<T: ?Sized> FakeBorrow<T> {
    pub(crate) fn new() -> Self {
        FakeBorrow(PhantomData)
    }
}
