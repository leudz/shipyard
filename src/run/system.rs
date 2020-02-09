use super::SystemData;
use crate::error;
use crate::world::World;

/// Trait to define systems.
///
/// `System::Data` can be:
/// * `&T` for a shared access to `T` storage
/// * `&mut T` for an exclusive access to `T` storage
/// * [Entities] for a shared access to the entity storage
/// * [EntitiesMut] for an exclusive reference to the entity storage
/// * [AllStorages] for an exclusive access to the storage of all components
/// * [ThreadPool] for a shared access to the `ThreadPool` used by the [World]
/// * [Unique]<&T> for a shared access to a `T` unique storage
/// * [Unique]<&mut T> for an exclusive access to a `T` unique storage
/// * `NonSend<&T>` for a shared access to a `T` storage where `T` isn't `Send`
/// * `NonSend<&mut T>` for an exclusive access to a `T` storage where `T` isn't `Send`
/// * `NonSync<&T>` for a shared access to a `T` storage where `T` isn't `Sync`
/// * `NonSync<&mut T>` for an exclusive access to a `T` storage where `T` isn't `Sync`
/// * `NonSendSync<&T>` for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
/// * `NonSendSync<&mut T>` for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`
///
/// [Unique] and `NonSend`/`NonSync`/`NonSendSync` can be used together to access a unique storage missing `Send` and/or `Sync` bound(s).
///
/// A tuple will allow multiple references.
/// ### Example
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
/// [EntitiesMut]: struct.EntitiesMut.html
/// [AllStorages]: struct.AllStorages.html
/// [ThreadPool]: struct.ThreadPool.html
/// [World]: struct.World.html
/// [Unique]: struct.Unique.html
pub trait System<'a> {
    type Data: SystemData<'a>;
    fn run(storage: <Self::Data as SystemData<'a>>::View);
}

pub(crate) trait Dispatch {
    fn try_dispatch(world: &World) -> Result<(), error::GetStorage>;
}

impl<T> Dispatch for T
where
    T: for<'a> System<'a>,
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
