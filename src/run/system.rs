use super::SystemData;
use crate::world::World;

/// Trait to define systems.
///
/// `System::Data` can be:
/// * `&T` for an immutable reference to `T` storage
/// * `&mut T` for a mutable reference to `T` storage
/// * [Entities] for a mutable reference to the entity storage
/// * [AllStorages] for a mutable reference to the storage of all components
/// * [ThreadPool] for an immutable reference to the `rayon::ThreadPool` used by the [World]
/// * [Not] can be used to filter out a component type
///
/// A tuple will allow multiple references.
/// # Example
/// ```
/// # use shipyard::*;
/// struct Single;
/// impl<'a> System<'a> for Single {
///     type Data = &'a usize;
///     fn run(&self, usizes: <Self::Data as SystemData>::View) {
///         // -- snip --
///     }
/// }
///
/// struct Double;
/// impl<'a> System<'a> for Double {
///     type Data = (&'a usize, &'a mut u32);
///     fn run(&self, (usizes, u32s): <Self::Data as SystemData>::View) {
///         // -- snip --
///     }
/// }
/// ```
/// [Entities]: struct.Entities.html
/// [AllStorages]: struct.AllStorages.html
/// [ThreadPool]: struct.ThreadPool.html
/// [World]: struct.World.html
/// [Not]: struct.Not.html
pub trait System<'a> {
    type Data: SystemData<'a>;
    fn run(&self, storage: <Self::Data as SystemData<'a>>::View);
}

pub(crate) trait Dispatch: Send + Sync {
    fn dispatch(&self, world: &World);
}

impl<T> Dispatch for T
where
    T: for<'a> System<'a> + Send + Sync,
{
    fn dispatch(&self, world: &World) {
        let entities = &world.entities;
        let storages = &world.storages;

        let mut borrows = Vec::new();

        let data = {
            #[cfg(feature = "parallel")]
            {
                let thread_pool = &world.thread_pool;
                // SAFE data is dropped before borrow
                unsafe {
                    <T::Data as SystemData<'_>>::borrow(
                        &mut borrows,
                        &entities,
                        &storages,
                        &thread_pool,
                    )
                }
            }
            #[cfg(not(feature = "parallel"))]
            {
                unsafe { <T::Data as SystemData<'_>>::borrow(&entities, &storages) }
            }
        };
        self.run(data);
    }
}
