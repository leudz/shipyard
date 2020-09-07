use core::marker::PhantomData;

/// Mimics an exclusive borrow of `T` without actually borrowing anything.  
/// Can be useful to correctly schedule `Sync` types.
///
/// Use [`Unique<T>`] for unique storage.
/// ### Example:
/// ```
/// use shipyard::{system, FakeBorrow, View, Workload, World};
///
/// fn display_first(_: View<usize>) {}
/// fn display_next(_: View<usize>) {}
///
/// let world = World::new();
///
/// Workload::builder("Display")
///     .with_system(system!(display_first))
///     .with_system(system!(|_: FakeBorrow<usize>| {}))
///     .with_system(system!(display_next))
///     .add_to_world(&world)
///     .unwrap();
/// ```
///
/// [`Unique<T>`]: struct.Unique.html
pub struct FakeBorrow<T: ?Sized>(PhantomData<T>);

impl<T: ?Sized> FakeBorrow<T> {
    pub(crate) fn new() -> Self {
        FakeBorrow(PhantomData)
    }
}
