mod system;

use crate::atomic_refcell::{AtomicRefCell, Borrow, Ref, RefMut};
use crate::component_storage::{AllStorages, AllStoragesViewMut};
use crate::entity::{Entities, EntitiesViewMut};
use crate::not::Not;
use crate::sparse_array::{View, ViewMut};
#[cfg(feature = "parallel")]
use rayon::ThreadPool;
use std::any::TypeId;
pub(crate) use system::Dispatch;
pub use system::{System, SystemData};

pub enum Either<T, U> {
    Single(T),
    Double(U),
    None,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mutation {
    Immutable,
    Mutable,
}

// `Run` make it possible to iterate over entities.
// It acts as an unnamed system.
pub trait Run<'a> {
    type Storage;
    #[cfg(feature = "parallel")]
    fn run<F: FnOnce(Self::Storage)>(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
        f: F,
    );

    #[cfg(not(feature = "parallel"))]
    fn run<F: FnOnce(Self::Storage)>(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        f: F,
    );
}

pub trait AbstractStorage<'a> {
    type AbstractStorage;
    /// # Safety `Self::AbstractStorage` has to be dropped before `Either`.
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>);

    /// # Safety `Self::AbstractStorage` has to be dropped before `Either`.
    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>);
    fn borrow_status() -> (TypeId, Mutation);
}

impl<'a> AbstractStorage<'a> for Entities {
    type AbstractStorage = EntitiesViewMut<'a>;
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        _: &'a AtomicRefCell<AllStorages>,
        _: &'a ThreadPool,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        let (entities, borrow) = RefMut::destructure(entities.try_borrow_mut().unwrap());
        (entities.view_mut(), Either::Single(borrow))
    }

    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        _: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        let (entities, borrow) = RefMut::destructure(entities.try_borrow_mut().unwrap());
        (entities.view_mut(), Either::Single(borrow))
    }
    fn borrow_status() -> (TypeId, Mutation) {
        (TypeId::of::<Entities>(), Mutation::Mutable)
    }
}

impl<'a> AbstractStorage<'a> for AllStorages {
    type AbstractStorage = AllStoragesViewMut<'a>;
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        _: &'a ThreadPool,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        let (storages, borrow) = RefMut::destructure(storages.try_borrow_mut().unwrap());
        (storages.view_mut(), Either::Single(borrow))
    }

    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        let (storages, borrow) = RefMut::destructure(storages.try_borrow_mut().unwrap());
        (storages.view_mut(), Either::Single(borrow))
    }
    fn borrow_status() -> (TypeId, Mutation) {
        (TypeId::of::<AllStorages>(), Mutation::Mutable)
    }
}

#[cfg(feature = "parallel")]
impl<'a> AbstractStorage<'a> for crate::ThreadPool {
    type AbstractStorage = &'a ThreadPool;
    unsafe fn borrow(
        _: &'a AtomicRefCell<Entities>,
        _: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        (thread_pool, Either::None)
    }
    fn borrow_status() -> (TypeId, Mutation) {
        (TypeId::of::<crate::ThreadPool>(), Mutation::Immutable)
    }
}

impl<'a, T: 'static> AbstractStorage<'a> for &T {
    type AbstractStorage = View<'a, T>;
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        _: &'a ThreadPool,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        let (storages, outer_borrow) = Ref::destructure(storages.try_borrow().unwrap());
        let (array, inner_borrow) =
            Ref::destructure(storages.0.get(&TypeId::of::<T>()).unwrap().array().unwrap());
        (array.view(), Either::Double([inner_borrow, outer_borrow]))
    }

    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        let (storages, outer_borrow) = Ref::destructure(storages.try_borrow().unwrap());
        let (array, inner_borrow) =
            Ref::destructure(storages.0.get(&TypeId::of::<T>()).unwrap().array().unwrap());
        (array.view(), Either::Double([inner_borrow, outer_borrow]))
    }
    fn borrow_status() -> (TypeId, Mutation) {
        (TypeId::of::<T>(), Mutation::Immutable)
    }
}

impl<'a, T: 'static> AbstractStorage<'a> for &mut T {
    type AbstractStorage = ViewMut<'a, T>;
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        _: &'a ThreadPool,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        let (storages, outer_borrow) = Ref::destructure(storages.try_borrow().unwrap());
        let (array, inner_borrow) = RefMut::destructure(
            storages
                .0
                .get(&TypeId::of::<T>())
                .unwrap()
                .array_mut()
                .unwrap(),
        );
        (
            array.view_mut(),
            Either::Double([inner_borrow, outer_borrow]),
        )
    }

    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        let (storages, outer_borrow) = Ref::destructure(storages.try_borrow().unwrap());
        let (array, inner_borrow) = RefMut::destructure(
            storages
                .0
                .get(&TypeId::of::<T>())
                .unwrap()
                .array_mut()
                .unwrap(),
        );
        (
            array.view_mut(),
            Either::Double([inner_borrow, outer_borrow]),
        )
    }
    fn borrow_status() -> (TypeId, Mutation) {
        (TypeId::of::<T>(), Mutation::Mutable)
    }
}

impl<'a, T: 'static> AbstractStorage<'a> for Not<&T> {
    type AbstractStorage = Not<View<'a, T>>;
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        let (view, borrow) = <&T as AbstractStorage>::borrow(entities, storages, thread_pool);
        (Not(view), borrow)
    }

    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        let (view, borrow) = <&T as AbstractStorage>::borrow(entities, storages);
        (Not(view), borrow)
    }
    fn borrow_status() -> (TypeId, Mutation) {
        (TypeId::of::<T>(), Mutation::Immutable)
    }
}

impl<'a, T: 'static> AbstractStorage<'a> for Not<&mut T> {
    type AbstractStorage = Not<ViewMut<'a, T>>;
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        let (view, borrow) = <&mut T as AbstractStorage>::borrow(entities, storages, thread_pool);
        (Not(view), borrow)
    }

    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        let (view, borrow) = <&mut T as AbstractStorage>::borrow(entities, storages);
        (Not(view), borrow)
    }
    fn borrow_status() -> (TypeId, Mutation) {
        (TypeId::of::<T>(), Mutation::Mutable)
    }
}

impl<'a, T: AbstractStorage<'a>> Run<'a> for T {
    type Storage = T::AbstractStorage;
    #[cfg(feature = "parallel")]
    fn run<F: FnOnce(Self::Storage)>(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
        f: F,
    ) {
        // SAFE storage is dropped before borrow
        let (storage, _borrow) = unsafe { T::borrow(entities, storages, thread_pool) };
        f(storage);
    }

    #[cfg(not(feature = "parallel"))]
    fn run<F: FnOnce(Self::Storage)>(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        f: F,
    ) {
        // SAFE storage is dropped before borrow
        let (storage, _borrow) = unsafe { T::borrow(entities, storages) };
        f(storage);
    }
}

impl<'a, T: AbstractStorage<'a>> Run<'a> for (T,) {
    type Storage = (T::AbstractStorage,);
    #[cfg(feature = "parallel")]
    fn run<F: FnOnce(Self::Storage)>(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
        f: F,
    ) {
        // SAFE storage is dropped before borrow
        let (storage, _borrow) = unsafe { T::borrow(entities, storages, thread_pool) };
        f((storage,));
    }

    #[cfg(not(feature = "parallel"))]
    fn run<F: FnOnce(Self::Storage)>(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        f: F,
    ) {
        // SAFE storage is dropped before borrow
        let (storage, _borrow) = unsafe { T::borrow(entities, storages) };
        f((storage,));
    }
}

macro_rules! impl_add_component {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: AbstractStorage<'a>,)+> Run<'a> for ($($type,)+) {
            type Storage = ($($type::AbstractStorage,)+);
            #[cfg(feature = "parallel")]
            fn run<Func: FnOnce(Self::Storage)>(
                entities: &'a AtomicRefCell<Entities>,
                storages: &'a AtomicRefCell<AllStorages>,
                thread_pool: &'a ThreadPool,
                f: Func
            ) {
                let mut i = 0;
                $({
                    let _: $type;
                    i += 1;
                })+
                let mut borrows = Vec::with_capacity(i);
                let storages = ($({
                    // SAFE storage is dropped before borrow
                    let (storage, borrow) = unsafe {$type::borrow(entities, storages, thread_pool)};
                    borrows.push(borrow);
                    storage
                },)+);
                f(storages);
            }

            #[cfg(not(feature = "parallel"))]
            fn run<Func: FnOnce(Self::Storage)>(
                entities: &'a AtomicRefCell<Entities>,
                storages: &'a AtomicRefCell<AllStorages>,
                f: Func
            ) {
                let mut i = 0;
                $({
                    let _: $type;
                    i += 1;
                })+
                let mut borrows = Vec::with_capacity(i);
                let storages = ($({
                    // SAFE storage is dropped before borrow
                    let (storage, borrow) = unsafe {$type::borrow(entities, storages)};
                    borrows.push(borrow);
                    storage
                },)+);
                f(storages);
            }
        }
    }
}

macro_rules! add_component {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_add_component![$(($type, $index))*];
        add_component![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_add_component![$(($type, $index))*];
    }
}

add_component![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
