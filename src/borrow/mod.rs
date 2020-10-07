mod all_storages;
mod fake_borrow;
#[cfg(feature = "non_send")]
mod non_send;
#[cfg(all(feature = "non_send", feature = "non_sync"))]
mod non_send_sync;
#[cfg(feature = "non_sync")]
mod non_sync;

pub use all_storages::AllStoragesBorrow;
pub use fake_borrow::FakeBorrow;
#[cfg(feature = "non_send")]
pub use non_send::NonSend;
#[cfg(all(feature = "non_send", feature = "non_sync"))]
pub use non_send_sync::NonSendSync;
#[cfg(feature = "non_sync")]
pub use non_sync::NonSync;

use crate::error;
use crate::storage::{AllStorages, Entities, Unique};
use crate::type_id::TypeId;
use crate::view::{
    AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut,
};
use crate::world::{TypeInfo, World};
use alloc::vec::Vec;
use core::any::type_name;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mutability {
    Shared,
    Exclusive,
}

pub trait Borrow<'a> {
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage>
    where
        Self: Sized;

    fn borrow_infos(infos: &mut Vec<TypeInfo>);

    fn is_send_sync() -> bool;
}

impl<'a> Borrow<'a> for () {
    #[inline]
    fn try_borrow(_: &'a World) -> Result<Self, error::GetStorage>
    where
        Self: Sized,
    {
        Ok(())
    }

    fn borrow_infos(_: &mut Vec<TypeInfo>) {}

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a> Borrow<'a> for AllStoragesViewMut<'a> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        AllStoragesViewMut::new(&world.all_storages)
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<AllStorages>(),
            mutability: Mutability::Exclusive,
            type_id: TypeId::of::<AllStorages>(),
            is_send: true,
            is_sync: true,
        });
    }

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a> Borrow<'a> for EntitiesView<'a> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        EntitiesView::from_ref(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Entities>(),
            mutability: Mutability::Shared,
            type_id: TypeId::of::<Entities>(),
            is_send: true,
            is_sync: true,
        });
    }

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a> Borrow<'a> for EntitiesViewMut<'a> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        EntitiesViewMut::from_ref(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Entities>(),
            mutability: Mutability::Exclusive,
            type_id: TypeId::of::<Entities>(),
            is_send: true,
            is_sync: true,
        });
    }

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for View<'a, T> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        View::from_ref(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Self>(),
            mutability: Mutability::Shared,
            type_id: TypeId::of::<T>(),
            is_send: true,
            is_sync: true,
        });
    }

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for ViewMut<'a, T> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        ViewMut::from_ref(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Self>(),
            mutability: Mutability::Exclusive,
            type_id: TypeId::of::<T>(),
            is_send: true,
            is_sync: true,
        });
    }

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for UniqueView<'a, T> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        UniqueView::from_ref(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Self>(),
            mutability: Mutability::Shared,
            type_id: TypeId::of::<Unique<T>>(),
            is_send: true,
            is_sync: true,
        });
    }

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a, T: 'static + Send + Sync> Borrow<'a> for UniqueViewMut<'a, T> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        UniqueViewMut::from_ref(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<Self>(),
            mutability: Mutability::Exclusive,
            type_id: TypeId::of::<Unique<T>>(),
            is_send: true,
            is_sync: true,
        });
    }

    fn is_send_sync() -> bool {
        true
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<View<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        View::from_ref_non_send(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSend)
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<View<'a, T>>(),
            mutability: Mutability::Shared,
            type_id: TypeId::of::<T>(),
            is_send: false,
            is_sync: true,
        });
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<ViewMut<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        ViewMut::from_ref_non_send(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSend)
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<ViewMut<'a, T>>(),
            mutability: Mutability::Exclusive,
            type_id: TypeId::of::<T>(),
            is_send: false,
            is_sync: true,
        });
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<UniqueView<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        UniqueView::from_ref(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSend)
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<UniqueView<'a, T>>(),
            mutability: Mutability::Exclusive,
            type_id: TypeId::of::<Unique<T>>(),
            is_send: false,
            is_sync: true,
        });
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_send")]
impl<'a, T: 'static + Sync> Borrow<'a> for NonSend<UniqueViewMut<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        UniqueViewMut::from_ref(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSend)
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<UniqueViewMut<'a, T>>(),
            mutability: Mutability::Exclusive,
            type_id: TypeId::of::<Unique<T>>(),
            is_send: false,
            is_sync: true,
        });
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<View<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        View::from_ref_non_sync(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSync)
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<View<'a, T>>(),
            mutability: Mutability::Shared,
            type_id: TypeId::of::<T>(),
            is_send: true,
            is_sync: false,
        });
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<ViewMut<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        ViewMut::from_ref_non_sync(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSync)
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<ViewMut<'a, T>>(),
            mutability: Mutability::Exclusive,
            type_id: TypeId::of::<T>(),
            is_send: true,
            is_sync: false,
        });
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<UniqueView<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        UniqueView::from_ref(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSync)
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<UniqueView<'a, T>>(),
            mutability: Mutability::Shared,
            type_id: TypeId::of::<Unique<T>>(),
            is_send: true,
            is_sync: false,
        });
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(feature = "non_sync")]
impl<'a, T: 'static + Send> Borrow<'a> for NonSync<UniqueViewMut<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        UniqueViewMut::from_ref(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSync)
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<UniqueViewMut<'a, T>>(),
            mutability: Mutability::Exclusive,
            type_id: TypeId::of::<Unique<T>>(),
            is_send: true,
            is_sync: false,
        });
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<View<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        View::from_ref_non_send_sync(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSendSync)
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<View<'a, T>>(),
            mutability: Mutability::Shared,
            type_id: TypeId::of::<T>(),
            is_send: false,
            is_sync: false,
        });
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<ViewMut<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        ViewMut::from_ref_non_send_sync(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSendSync)
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<ViewMut<'a, T>>(),
            mutability: Mutability::Exclusive,
            type_id: TypeId::of::<T>(),
            is_send: false,
            is_sync: false,
        });
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<UniqueView<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        UniqueView::from_ref(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSendSync)
    }
    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<UniqueView<'a, T>>(),
            mutability: Mutability::Exclusive,
            type_id: TypeId::of::<Unique<T>>(),
            is_send: false,
            is_sync: false,
        });
    }

    fn is_send_sync() -> bool {
        false
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<'a, T: 'static> Borrow<'a> for NonSendSync<UniqueViewMut<'a, T>> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        UniqueViewMut::from_ref(
            world
                .all_storages
                .try_borrow()
                .map_err(error::GetStorage::AllStoragesBorrow)?,
        )
        .map(NonSendSync)
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<UniqueViewMut<'a, T>>(),
            mutability: Mutability::Exclusive,
            type_id: TypeId::of::<Unique<T>>(),
            is_send: false,
            is_sync: false,
        });
    }

    fn is_send_sync() -> bool {
        false
    }
}

impl<T: 'static> Borrow<'_> for FakeBorrow<T> {
    #[inline]
    fn try_borrow(_: &World) -> Result<Self, error::GetStorage> {
        Ok(FakeBorrow::new())
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        infos.push(TypeInfo {
            name: type_name::<T>(),
            mutability: Mutability::Exclusive,
            type_id: TypeId::of::<T>(),
            is_send: true,
            is_sync: true,
        });
    }

    fn is_send_sync() -> bool {
        true
    }
}

impl<'a, T: Borrow<'a>> Borrow<'a> for Option<T> {
    #[inline]
    fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        Ok(T::try_borrow(world).ok())
    }

    fn borrow_infos(infos: &mut Vec<TypeInfo>) {
        T::borrow_infos(infos);
    }

    fn is_send_sync() -> bool {
        T::is_send_sync()
    }
}

macro_rules! impl_borrow {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: Borrow<'a>),+> Borrow<'a> for ($($type,)+) {
            #[inline]
            fn try_borrow(world: &'a World) -> Result<Self, error::GetStorage> {
                Ok(($(
                    <$type as Borrow>::try_borrow(world)?,
                )+))
            }

            fn borrow_infos(infos: &mut Vec<TypeInfo>) {
                $(
                    $type::borrow_infos(infos);
                )+
            }

            fn is_send_sync() -> bool {
                $($type::is_send_sync())&&+
            }
        }
    }
}

macro_rules! borrow {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_borrow![$(($type, $index))*];
        borrow![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_borrow![$(($type, $index))*];
    }
}

borrow![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
