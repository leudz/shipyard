use super::{AbstractStorage, Either, Mutation};
use crate::atomic_refcell::{AtomicRefCell, Borrow};
use crate::component_storage::AllStorages;
use crate::entity::Entities;
use crate::world::World;
#[cfg(feature = "parallel")]
use rayon::ThreadPool;
use std::any::TypeId;

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

        let (data, _borrow) = {
            #[cfg(feature = "parallel")]
            {
                let thread_pool = &world.thread_pool;
                // SAFE data is dropped before borrow
                unsafe { <T::Data as SystemData<'_>>::borrow(&entities, &storages, &thread_pool) }
            }
            #[cfg(not(feature = "parallel"))]
            {
                unsafe { <T::Data as SystemData<'_>>::borrow(&entities, &storages) }
            }
        };
        self.run(data);
    }
}

pub trait SystemData<'a> {
    type View;
    /// # Safety `Self::Data` has to be dropped before `Either`.
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> (Self::View, Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>);
    /// # Safety `Self::Data` has to be dropped before `Either`.
    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::View, Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>);
    fn borrow_status() -> Vec<(TypeId, Mutation)>;
}

impl<'a, T: AbstractStorage<'a>> SystemData<'a> for T {
    type View = T::AbstractStorage;
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> (Self::View, Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>) {
        let (data, borrow) = <T as AbstractStorage<'a>>::borrow(entities, storages, thread_pool);
        (data, vec![borrow])
    }
    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::View, Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>) {
        let (data, borrow) = <T as AbstractStorage<'a>>::borrow(entities, storages);
        (data, vec![borrow])
    }
    fn borrow_status() -> Vec<(TypeId, Mutation)> {
        vec![<T as AbstractStorage>::borrow_status()]
    }
}

impl<'a, T: AbstractStorage<'a>> SystemData<'a> for (T,) {
    type View = (T::AbstractStorage,);
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> (Self::View, Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>) {
        let (data, borrow) = T::borrow(entities, storages, thread_pool);
        ((data,), vec![borrow])
    }
    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::View, Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>) {
        let (data, borrow) = T::borrow(entities, storages);
        ((data,), vec![borrow])
    }
    fn borrow_status() -> Vec<(TypeId, Mutation)> {
        vec![<T as AbstractStorage>::borrow_status()]
    }
}

macro_rules! impl_system_data {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: AbstractStorage<'a>),+> SystemData<'a> for ($($type,)+) {
            type View = ($($type::AbstractStorage,)+);
            #[cfg(feature = "parallel")]
            unsafe fn borrow(
                entities: &'a AtomicRefCell<Entities>,
                storages: &'a AtomicRefCell<AllStorages>,
                thread_pool: &'a ThreadPool,
            ) -> (Self::View, Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>) {
                let mut borrows = Vec::new();
                (($({
                    let (data, borrow) = <$type as AbstractStorage<'a>>::borrow(entities, storages, thread_pool);
                    borrows.push(borrow);
                    data
                },)+), borrows)
            }
            #[cfg(not(feature = "parallel"))]
            unsafe fn borrow(
                entities: &'a AtomicRefCell<Entities>,
                storages: &'a AtomicRefCell<AllStorages>,
            ) -> (Self::View, Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>) {
                let mut borrows = Vec::new();
                (($({
                    let (data, borrow) = <$type as AbstractStorage<'a>>::borrow(entities, storages);
                    borrows.push(borrow);
                    data
                },)+), borrows)
            }
            fn borrow_status() -> Vec<(TypeId, Mutation)> {
                vec![$(<$type as AbstractStorage>::borrow_status()),+]
            }
        }
    }
}

macro_rules! system_data {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_system_data![$(($type, $index))*];
        system_data![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_system_data![$(($type, $index))*];
    }
}

system_data![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
