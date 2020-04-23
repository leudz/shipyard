/// Reduce boilerplace to add a system to a workload and make it less error prone.
///
/// ### Example
/// ```
/// use shipyard::{system, EntitiesViewMut, IntoIter, Shiperator, View, ViewMut, World};
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
/// world
///     .add_workload("Add & Check")
///     .with_system(system!(add))
///     .with_system(system!(check))
///     .build();
///
/// world.run_default();
/// ```
#[macro_export]
macro_rules! system {
    ($function: expr) => {{
        (
            |world: &shipyard::World| world.try_run($function).map(drop),
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
/// use shipyard::{try_system, EntitiesViewMut, World};
///
/// struct TerribleError;
///
/// fn my_sys(mut entities: EntitiesViewMut) -> Result<(), TerribleError> {
///     Err(TerribleError)
/// }
///
/// let world = World::new();
///
/// world
///     .add_workload("May fail")
///     .with_system(try_system!(my_sys))
///     .build();
///
/// if let Some(error) = world.try_run_default().err().unwrap().custom_error() {
///     let terrible_error = error.downcast::<TerribleError>();
/// }
/// ```
#[macro_export]
macro_rules! try_system {
    ($function: expr) => {{
        (
            |world: &shipyard::World| {
                world
                    .try_run($function)?
                    .map_err(shipyard::error::Run::from_custom)
            },
            $function,
        )
    }};
}
