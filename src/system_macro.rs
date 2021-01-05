/// Reduces boilerplate to add a system to a workload and make it less error prone.
///
/// ### Example
/// ```
/// use shipyard::{system, IntoIter, View, ViewMut, Workload, WorkloadSystem, World};
///
/// fn add(mut usizes: ViewMut<usize>, u32s: View<u32>) {
///     for (mut x, &y) in (&mut usizes, &u32s).iter() {
///         *x += y as usize;
///     }
/// }
///
/// fn check(usizes: View<usize>) {
///     let mut iter = usizes.iter();
///     assert_eq!(iter.next(), Some(&1));
///     assert_eq!(iter.next(), Some(&5));
///     assert_eq!(iter.next(), Some(&9));
/// }
///
/// let mut world = World::new();
///
/// world.add_entity((0usize, 1u32));
/// world.add_entity((2usize, 3u32));
/// world.add_entity((4usize, 5u32));
///
/// let mut workload = Workload::builder("Add & Check");
///
/// // Without macro
/// workload.with_system(WorkloadSystem::new(|world| world.run(add), add).unwrap());
///
/// // With macro
/// workload.with_system(system!(check));
///
/// workload.add_to_world(&world).unwrap();
///
/// world.run_default().unwrap();
/// ```
#[macro_export]
macro_rules! system {
    ($function: expr) => {{
        $crate::WorkloadSystem::new(
            |world: &$crate::World| world.run($function).map(drop),
            $function,
        )
        .unwrap()
    }};
}

/// Reduces boilerplate to add a fallible system to a workload and make it less error prone.  
///
/// This macro only works with systems returning a `Result`.
///
/// ### Example
/// ```
/// # #[cfg(feature = "std")]
/// # {
/// use shipyard::{error::Run, error::RunWorkload, try_system, Workload, WorkloadSystem, World};
/// use std::error::Error;
/// use std::fmt::{Debug, Display, Formatter};
///
/// #[derive(Debug)]
/// struct TerribleError;
///
/// impl Display for TerribleError {
///     fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
///         Debug::fmt(self, fmt)
///     }
/// }
/// impl Error for TerribleError {}
///
/// fn ok_sys() -> Result<(), TerribleError> {
///     Ok(())
/// }
///
/// fn err_sys() -> Result<(), TerribleError> {
///     Err(TerribleError)
/// }
///
/// let world = World::new();
///
/// let mut workload = Workload::builder("May fail");
///
/// // Without macro
/// workload.with_system(WorkloadSystem::new(|world| world.run(ok_sys)?.map_err(Run::from_custom), ok_sys).unwrap());
///
/// // With macro
/// workload.with_system(try_system!(err_sys));
///
/// workload.add_to_world(&world).unwrap();
///
/// assert!(world
///     .run_default()
///     .unwrap_err()
///     .custom_error()
///     .unwrap()
///     .is::<TerribleError>());
/// # }
/// ```
#[macro_export]
macro_rules! try_system {
    ($function: expr) => {{
        $crate::WorkloadSystem::new(
            |world: &$crate::World| {
                world
                    .run($function)?
                    .map_err($crate::error::Run::from_custom)
            },
            $function,
        )
        .unwrap()
    }};
}
