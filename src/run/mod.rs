mod system;

use crate::atomic_refcell::{AtomicRefCell, Borrow, Ref, RefMut};
use crate::component_storage::{AllStorages, AllStoragesViewMut};
use crate::entity::{Entities, EntitiesMut, EntitiesView, EntitiesViewMut};
use crate::not::Not;
use crate::sparse_array::{View, ViewMut};
#[cfg(feature = "parallel")]
use rayon::ThreadPool;
use std::any::TypeId;
pub(crate) use system::Dispatch;
pub use system::System;

pub enum Either<T, U> {
    Single(T),
    Double(U),
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

pub trait SystemData<'a> {
    type View;
    /// # Safety `Self::View` has to be dropped before `Either`.
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> Self::View;

    /// # Safety `Self::View` has to be dropped before `Either`.
    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> Self::View;
    fn borrow_status(status: &mut Vec<(TypeId, Mutation)>);
}

impl<'a> SystemData<'a> for Entities {
    type View = EntitiesView<'a>;
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        entities: &'a AtomicRefCell<Entities>,
        _: &'a AtomicRefCell<AllStorages>,
        _: &'a ThreadPool,
    ) -> Self::View {
        let (entities, borrow) = Ref::destructure(entities.try_borrow().unwrap());
        borrows.push(Either::Single(borrow));
        entities.view()
    }

    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        entities: &'a AtomicRefCell<Entities>,
        _: &'a AtomicRefCell<AllStorages>,
    ) -> Self::View {
        let (entities, borrow) = Ref::destructure(entities.try_borrow().unwrap());
        borrows.push(Either::Single(borrow));
        entities.view()
    }
    fn borrow_status(status: &mut Vec<(TypeId, Mutation)>) {
        status.push((TypeId::of::<Entities>(), Mutation::Immutable));
    }
}

impl<'a> SystemData<'a> for EntitiesMut {
    type View = EntitiesViewMut<'a>;
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        entities: &'a AtomicRefCell<Entities>,
        _: &'a AtomicRefCell<AllStorages>,
        _: &'a ThreadPool,
    ) -> Self::View {
        let (entities, borrow) = RefMut::destructure(entities.try_borrow_mut().unwrap());
        borrows.push(Either::Single(borrow));
        entities.view_mut()
    }

    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        entities: &'a AtomicRefCell<Entities>,
        _: &'a AtomicRefCell<AllStorages>,
    ) -> Self::View {
        let (entities, borrow) = RefMut::destructure(entities.try_borrow_mut().unwrap());
        borrows.push(Either::Single(borrow));
        entities.view_mut()
    }
    fn borrow_status(status: &mut Vec<(TypeId, Mutation)>) {
        status.push((TypeId::of::<Entities>(), Mutation::Mutable));
    }
}

impl<'a> SystemData<'a> for AllStorages {
    type View = AllStoragesViewMut<'a>;
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        _: &'a ThreadPool,
    ) -> Self::View {
        let (storages, borrow) = RefMut::destructure(storages.try_borrow_mut().unwrap());
        borrows.push(Either::Single(borrow));
        storages.view_mut()
    }

    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> Self::View {
        let (storages, borrow) = RefMut::destructure(storages.try_borrow_mut().unwrap());
        borrows.push(Either::Single(borrow));
        storages.view_mut()
    }
    fn borrow_status(status: &mut Vec<(TypeId, Mutation)>) {
        status.push((TypeId::of::<AllStorages>(), Mutation::Mutable));
    }
}

#[cfg(feature = "parallel")]
impl<'a> SystemData<'a> for crate::ThreadPool {
    type View = &'a ThreadPool;
    unsafe fn borrow(
        _: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        _: &'a AtomicRefCell<Entities>,
        _: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> Self::View {
        thread_pool
    }
    fn borrow_status(status: &mut Vec<(TypeId, Mutation)>) {
        status.push((TypeId::of::<crate::ThreadPool>(), Mutation::Immutable));
    }
}

impl<'a, T: 'static> SystemData<'a> for &T {
    type View = View<'a, T>;
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        _: &'a ThreadPool,
    ) -> Self::View {
        let (storages, outer_borrow) = Ref::destructure(storages.try_borrow().unwrap());
        let (array, inner_borrow) =
            Ref::destructure(storages.0.get(&TypeId::of::<T>()).unwrap().array().unwrap());
        borrows.push(Either::Double([inner_borrow, outer_borrow]));
        array.view()
    }

    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> Self::View {
        let (storages, outer_borrow) = Ref::destructure(storages.try_borrow().unwrap());
        let (array, inner_borrow) =
            Ref::destructure(storages.0.get(&TypeId::of::<T>()).unwrap().array().unwrap());
        borrows.push(Either::Double([inner_borrow, outer_borrow]));
        array.view()
    }
    fn borrow_status(status: &mut Vec<(TypeId, Mutation)>) {
        status.push((TypeId::of::<T>(), Mutation::Immutable));
    }
}

impl<'a, T: 'static> SystemData<'a> for &mut T {
    type View = ViewMut<'a, T>;
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        _: &'a ThreadPool,
    ) -> Self::View {
        let (storages, outer_borrow) = Ref::destructure(storages.try_borrow().unwrap());
        let (array, inner_borrow) = RefMut::destructure(
            storages
                .0
                .get(&TypeId::of::<T>())
                .unwrap()
                .array_mut()
                .unwrap(),
        );
        borrows.push(Either::Double([inner_borrow, outer_borrow]));
        array.view_mut()
    }

    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> Self::View {
        let (storages, outer_borrow) = Ref::destructure(storages.try_borrow().unwrap());
        let (array, inner_borrow) = RefMut::destructure(
            storages
                .0
                .get(&TypeId::of::<T>())
                .unwrap()
                .array_mut()
                .unwrap(),
        );
        borrows.push(Either::Double([inner_borrow, outer_borrow]));
        array.view_mut()
    }
    fn borrow_status(status: &mut Vec<(TypeId, Mutation)>) {
        status.push((TypeId::of::<T>(), Mutation::Mutable));
    }
}

impl<'a, T: 'static> SystemData<'a> for Not<&T> {
    type View = Not<View<'a, T>>;
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> Self::View {
        let view = <&T as SystemData>::borrow(borrows, entities, storages, thread_pool);
        Not(view)
    }

    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> Self::View {
        let view = <&T as SystemData>::borrow(borrows, entities, storages);
        Not(view)
    }
    fn borrow_status(status: &mut Vec<(TypeId, Mutation)>) {
        <&T as SystemData>::borrow_status(status)
    }
}

impl<'a, T: 'static> SystemData<'a> for Not<&mut T> {
    type View = Not<ViewMut<'a, T>>;
    #[cfg(feature = "parallel")]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
    ) -> Self::View {
        let view = <&mut T as SystemData>::borrow(borrows, entities, storages, thread_pool);
        Not(view)
    }

    #[cfg(not(feature = "parallel"))]
    unsafe fn borrow(
        borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> Self::View {
        let view = <&mut T as SystemData>::borrow(borrows, entities, storages);
        Not(view)
    }
    fn borrow_status(status: &mut Vec<(TypeId, Mutation)>) {
        <&mut T as SystemData>::borrow_status(status)
    }
}

impl<'a, T: SystemData<'a>> Run<'a> for T {
    type Storage = T::View;
    #[cfg(feature = "parallel")]
    fn run<F: FnOnce(Self::Storage)>(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        thread_pool: &'a ThreadPool,
        f: F,
    ) {
        let mut borrows = Vec::new();
        // SAFE storage is dropped before borrows
        let storage = unsafe { T::borrow(&mut borrows, entities, storages, thread_pool) };
        f(storage);
    }

    #[cfg(not(feature = "parallel"))]
    fn run<F: FnOnce(Self::Storage)>(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        f: F,
    ) {
        let mut borrows = Vec::new();
        // SAFE storage is dropped before borrow
        let storage = unsafe { T::borrow(&mut borrows, entities, storages) };
        f(storage);
    }
}

macro_rules! impl_add_component {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: SystemData<'a>),+> SystemData<'a> for ($($type,)+) {
            type View = ($($type::View,)+);
            /// # Safety `Self::View` has to be dropped before `Either`.
            #[cfg(feature = "parallel")]
            unsafe fn borrow(
                borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
                entities: &'a AtomicRefCell<Entities>,
                storages: &'a AtomicRefCell<AllStorages>,
                thread_pool: &'a ThreadPool,
            ) -> Self::View {
                ($(
                    $type::borrow(borrows, entities, storages, thread_pool),
                )+)
            }

            /// # Safety `Self::View` has to be dropped before `Either`.
            #[cfg(not(feature = "parallel"))]
            unsafe fn borrow(
                borrows: &mut Vec<Either<Borrow<'a>, [Borrow<'a>; 2]>>,
                entities: &'a AtomicRefCell<Entities>,
                storages: &'a AtomicRefCell<AllStorages>,
            ) -> Self::View {
                ($(
                    $type::borrow(borrows, entities, storages),
                )+)
            }
            fn borrow_status(status: &mut Vec<(TypeId, Mutation)>) {
                $(
                    $type::borrow_status(status);
                )+
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

add_component![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
