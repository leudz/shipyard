use super::SystemData;
use crate::error;
use crate::world::World;

/// Trait to define systems.
///
/// `System::Data` can be:
/// * `&T` for an immutable reference to `T` storage
/// * `&mut T` for a mutable reference to `T` storage
/// * [Entities] for an immmutable reference to the entity storage
/// * [EntitiesMut] for a mutable reference to the entity storage
/// * [AllStorages] for a mutable reference to the storage of all components
/// * [ThreadPool] for an immutable reference to the `rayon::ThreadPool` used by the [World]
/// * [Not] can be used to filter out a component type
///
/// A tuple will allow multiple references.
/// # Example
/// ```
/// # use shipyard::prelude::*;
/// struct Single;
/// impl<'a> System<'a> for Single {
///     type Data = &'a usize;
///     fn run(usizes: <Self::Data as SystemData>::View) {
///         // -- snip --
///     }
/// }
///
/// struct Double;
/// impl<'a> System<'a> for Double {
///     type Data = (&'a usize, &'a mut u32);
///     fn run((usizes, u32s): <Self::Data as SystemData>::View) {
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
    fn run(storage: <Self::Data as SystemData<'a>>::View);
}

pub(crate) trait Dispatch: Send + Sync {
    fn try_dispatch(world: &World) -> Result<(), error::GetStorage>;
}

impl<T> Dispatch for T
where
    T: for<'a> System<'a> + Send + Sync,
{
    fn try_dispatch(world: &World) -> Result<(), error::GetStorage> {
        let storages = &world.all_storages;

        let data = {
            #[cfg(feature = "parallel")]
            {
                let thread_pool = &world.thread_pool;
                <T::Data as SystemData>::try_borrow(&storages, &thread_pool)?
            }
            #[cfg(not(feature = "parallel"))]
            {
                <T::Data as SystemData>::try_borrow(&storages)?
            }
        };

        T::run(data);

        Ok(())
    }
}
