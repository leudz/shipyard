use crate::atomic_refcell::{AtomicRefCell, Borrow, Ref, RefMut};
use crate::component_storage::{AllStorages, AllStoragesViewMut};
use crate::entity::{Entities, EntityViewMut};
use crate::not::Not;
use crate::sparse_array::{View, ViewMut};
use std::any::TypeId;

// `Run` make it possible to iterate over entities
// It acts like an unnamed system

pub enum Either<T, U> {
    Single(T),
    Double(U),
}

pub trait Run<'a> {
    type Storage;
    fn run<F: FnOnce(Self::Storage)>(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
        f: F,
    );
}

pub trait AbstractStorage<'a> {
    type AbstractStorage;
    fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>);
}

impl<'a> AbstractStorage<'a> for Entities {
    type AbstractStorage = EntityViewMut<'a>;
    fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        _: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        // SAFE the reference is dropped before the borrow
        let (entities, borrow) = unsafe { RefMut::destructure(entities.try_borrow_mut().unwrap()) };
        (entities.view_mut(), Either::Single(borrow))
    }
}

impl<'a> AbstractStorage<'a> for AllStorages {
    type AbstractStorage = AllStoragesViewMut<'a>;
    fn borrow(
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        // SAFE the reference is dropped before the borrow
        let (storages, borrow) = unsafe { RefMut::destructure(storages.try_borrow_mut().unwrap()) };
        (storages.view_mut(), Either::Single(borrow))
    }
}

impl<'a, T: 'static> AbstractStorage<'a> for &T {
    type AbstractStorage = View<'a, T>;
    fn borrow(
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        // SAFE the reference is dropped before the borrow
        let (storages, outer_borrow) = unsafe { Ref::destructure(storages.try_borrow().unwrap()) };
        // SAFE the reference is dropped before the borrow and `inner_borrow` before `outer_borrow`
        let (array, inner_borrow) = unsafe {
            Ref::destructure(storages.0.get(&TypeId::of::<T>()).unwrap().array().unwrap())
        };
        (array.view(), Either::Double([inner_borrow, outer_borrow]))
    }
}

impl<'a, T: 'static> AbstractStorage<'a> for &mut T {
    type AbstractStorage = ViewMut<'a, T>;
    fn borrow(
        _: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        // SAFE the reference is dropped before the borrow
        let (storages, outer_borrow) = unsafe { Ref::destructure(storages.try_borrow().unwrap()) };
        // SAFE the reference is dropped before the borrow and `inner_borrow` before `outer_borrow`
        let (array, inner_borrow) = unsafe {
            RefMut::destructure(
                storages
                    .0
                    .get(&TypeId::of::<T>())
                    .unwrap()
                    .array_mut()
                    .unwrap(),
            )
        };
        (
            array.view_mut(),
            Either::Double([inner_borrow, outer_borrow]),
        )
    }
}

impl<'a, T: 'static> AbstractStorage<'a> for Not<&T> {
    type AbstractStorage = Not<View<'a, T>>;
    fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        let (view, borrow) = <&T>::borrow(entities, storages);
        (Not(view), borrow)
    }
}

impl<'a, T: 'static> AbstractStorage<'a> for Not<&mut T> {
    type AbstractStorage = Not<ViewMut<'a, T>>;
    fn borrow(
        entities: &'a AtomicRefCell<Entities>,
        storages: &'a AtomicRefCell<AllStorages>,
    ) -> (Self::AbstractStorage, Either<Borrow<'a>, [Borrow<'a>; 2]>) {
        let (view, borrow) = <&mut T>::borrow(entities, storages);
        (Not(view), borrow)
    }
}

macro_rules! impl_add_component {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: AbstractStorage<'a>,)+> Run<'a> for ($($type,)+) {
            type Storage = ($($type::AbstractStorage,)+);
            fn run<F: FnOnce(Self::Storage)>(entities: &'a AtomicRefCell<Entities>, storages: &'a AtomicRefCell<AllStorages>, f: F) {
                let mut i = 0;
                $({
                    let _: $type;
                    i += 1;
                })+
                let mut borrows = Vec::with_capacity(i);
                let storages = ($({
                    let (storage, borrow) = $type::borrow(entities, storages);
                    borrows.push(borrow);
                    storage
                },)+);
                f(storages);
            }
        }
    }
}

macro_rules! add_component {
    ($(($left_type: ident, $left_index: tt))*;($type1: ident, $index1: tt) $(($type: ident, $index: tt))*) => {
        impl_add_component![$(($left_type, $left_index))*];
        add_component![$(($left_type, $left_index))* ($type1, $index1); $(($type, $index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_add_component![$(($type, $index))*];
    }
}

add_component![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];
