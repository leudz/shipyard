/// Reduce boilerplace to add a system to a workload and make it less error prone.
///
/// ### Example
/// ```
/// use shipyard::{system, EntitiesViewMut, IntoIter, Shiperator, View, ViewMut, Workload, World};
///
/// fn add(mut usizes: ViewMut<usize>, u32s: View<u32>) {
///     for (x, &y) in (&mut usizes, &u32s).iter() {
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
/// let world = World::new();
///
/// world.run(
///     |mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
///         entities.add_entity((&mut usizes, &mut u32s), (0, 1));
///         entities.add_entity((&mut usizes, &mut u32s), (2, 3));
///         entities.add_entity((&mut usizes, &mut u32s), (4, 5));
///     },
/// );
///
/// Workload::builder("Add & Check")
///     .with_system(system!(add))
///     .with_system(system!(check))
///     .add_to_world(&world)
///     .unwrap();
///
/// world.run_default();
/// ```
#[macro_export]
macro_rules! system {
    ($function: expr) => {{
        (
            |world: &$crate::World| world.try_run($function).map(drop),
            $function,
        )
    }};
}

/// Reduce boilerplace to add a fallible system to a workload and make it less error prone.  
///
/// This macro only works with systems returning a `Result`.
///
/// ### Example
/// ```
/// #[cfg(feature = "std")]
/// {
/// use shipyard::{error::RunWorkload, try_system, EntitiesViewMut, Workload, World};
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
/// fn my_sys(mut entities: EntitiesViewMut) -> Result<(), TerribleError> {
///     Err(TerribleError)
/// }
///
/// fn main() {
///     let world = World::new();
///     Workload::builder("May fail")
///         .with_system(try_system!(my_sys))
///         .add_to_world(&world)
///         .unwrap();
///     match world.try_run_default().map_err(RunWorkload::custom_error) {
///         Err(Some(error)) => {
///             assert!(error.is::<TerribleError>());
///         }
///         _ => {}
///     }
/// }
/// }
/// ```
#[macro_export]
macro_rules! try_system {
    ($function: expr) => {{
        (
            |world: &$crate::World| {
                world
                    .try_run($function)?
                    .map_err($crate::error::Run::from_custom)
            },
            $function,
        )
    }};
}
