use super::{AbstractStorage, Either, Mutation};
use crate::atomic_refcell::{AtomicRefCell, Borrow};
use crate::component_storage::AllStorages;
use crate::entity::Entities;
use crate::world::World;
use rayon::ThreadPool;
use std::any::TypeId;

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
        let thread_pool = &world.thread_pool;
        // SAFE data is dropped before borrow
        let (data, _borrow) =
            unsafe { <T::Data as SystemData<'_>>::borrow(&entities, &storages, &thread_pool) };
        self.run(data);
    }
}

pub trait SystemData<'a> {
    type View;
    /// # Safety `Self::Data` has to be dropped before `Either`.
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> (Self::View, Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>);
    fn borrow_status() -> Vec<(TypeId, Mutation)>;
}

impl<'a, T: AbstractStorage<'a>> SystemData<'a> for T {
    type View = T::AbstractStorage;
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> (Self::View, Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>) {
        let (data, borrow) = <T as AbstractStorage<'a>>::borrow(entities, storages, thread_pool);
        (data, vec![borrow])
    }
    fn borrow_status() -> Vec<(TypeId, Mutation)> {
        vec![<T as AbstractStorage>::borrow_status()]
    }
}

impl<'a, T: AbstractStorage<'a>> SystemData<'a> for (T,) {
    type View = (T::AbstractStorage,);
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> (Self::View, Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>) {
        let (data, borrow) = T::borrow(entities, storages, thread_pool);
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

system_data![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];
