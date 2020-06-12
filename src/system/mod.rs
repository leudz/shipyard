mod all_storages;

pub use all_storages::AllSystem;

use crate::atomic_refcell::AtomicRefCell;
use crate::borrow::Mutation;
use crate::borrow::{Borrow, DynamicBorrow};
use crate::error;
use crate::storage::{AllStorages, StorageId};
use crate::view::{DynamicView, DynamicViewMut};
use alloc::vec::Vec;
use core::convert::TryFrom;

pub struct Nothing;

pub trait System<'s, Data, B, R> {
    fn run(self, data: Data, b: B) -> R;
    fn try_borrow(
        &self,
        all_storages: &'s AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] thread_pool: &'s rayon::ThreadPool,
    ) -> Result<B, error::GetStorage>;

    fn borrow_infos(&self, infos: &mut Vec<(StorageId, Mutation)>);

    fn is_send_sync(&self) -> bool;
}

// Nothing has to be used and not () to not conflict where A = ()
impl<'s, R, F> System<'s, (), Nothing, R> for F
where
    F: FnOnce() -> R,
{
    fn run(self, _: (), _: Nothing) -> R {
        (self)()
    }
    fn try_borrow(
        &self,
        _: &'s AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'s rayon::ThreadPool,
    ) -> Result<Nothing, error::GetStorage> {
        Ok(Nothing)
    }

    fn borrow_infos(&self, _: &mut Vec<(StorageId, Mutation)>) {}

    fn is_send_sync(&self) -> bool {
        true
    }
}

// Nothing has to be used and not () to not conflict where A = ()
impl<'s, Data, R, F> System<'s, (Data,), Nothing, R> for F
where
    F: FnOnce(Data) -> R,
{
    fn run(self, (data,): (Data,), _: Nothing) -> R {
        (self)(data)
    }
    fn try_borrow(
        &self,
        _: &'s AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'s rayon::ThreadPool,
    ) -> Result<Nothing, error::GetStorage> {
        Ok(Nothing)
    }

    fn borrow_infos(&self, _: &mut Vec<(StorageId, Mutation)>) {}

    fn is_send_sync(&self) -> bool {
        true
    }
}

/// Information necessary to define a custom component
#[derive(Debug, Clone)]
pub struct CustomComponent {
    /// The size of the components in bytes
    pub size: CustomComponentSize,
    /// The unique identifier for the component
    pub id: u64,
}

#[derive(Debug, Clone)]
pub enum CustomComponentSize {
    D16,
    D32,
}

#[derive(Debug, Clone)]
pub struct CustomComponentSizeError {
    size: u64,
}

impl TryFrom<u64> for CustomComponentSize {
    type Error = CustomComponentSizeError;

    fn try_from(size: u64) -> Result<Self, Self::Error> {
        match size {
            x if x <= 16 => Ok(Self::D16),
            x if x <= 32 => Ok(Self::D32),
            _ => Err(CustomComponentSizeError { size }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CustomComponentBorrowIntent {
    pub component: CustomComponent,
    pub mutation: Mutation,
}

type DynamicSystemBorrow<'a> = Vec<Box<dyn DynamicBorrow<'a> + 'a>>;

#[derive(Debug, Clone)]
pub struct DynamicSystem<'a, Data, R> {
    pub data: Data,
    pub system_fn: fn(Data, DynamicSystemBorrow<'a>) -> R,
    pub borrow_intents: Vec<CustomComponentBorrowIntent>,
}

impl<'s, Data, R> System<'s, Data, DynamicSystemBorrow<'s>, R> for DynamicSystem<'s, Data, R> {
    fn run(self, data: Data, borrow: DynamicSystemBorrow<'s>) -> R {
        (self.system_fn)(data, borrow)
    }

    fn try_borrow(
        &self,
        all_storages: &'s AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'s rayon::ThreadPool,
    ) -> Result<DynamicSystemBorrow<'s>, error::GetStorage> {
        Ok(self
            .borrow_intents
            .iter()
            .map(
                |intent| -> Result<Box<dyn DynamicBorrow<'s>>, error::GetStorage> {
                    match intent.component.size {
                        CustomComponentSize::D16 => match intent.mutation {
                            Mutation::Shared => {
                                Ok(Box::new(DynamicView::<[u8; 16]>::from_storage_atomic_ref(
                                    all_storages
                                        .try_borrow()
                                        .map_err(error::GetStorage::AllStoragesBorrow)?,
                                    StorageId::Custom(intent.component.id),
                                )?))
                            }
                            Mutation::Unique => Ok(Box::new(
                                DynamicViewMut::<[u8; 16]>::from_storage_atomic_ref(
                                    all_storages
                                        .try_borrow()
                                        .map_err(error::GetStorage::AllStoragesBorrow)?,
                                    StorageId::Custom(intent.component.id),
                                )?,
                            )),
                        },
                        CustomComponentSize::D32 => match intent.mutation {
                            Mutation::Shared => {
                                Ok(Box::new(DynamicView::<[u8; 32]>::from_storage_atomic_ref(
                                    all_storages
                                        .try_borrow()
                                        .map_err(error::GetStorage::AllStoragesBorrow)?,
                                    StorageId::Custom(intent.component.id),
                                )?))
                            }
                            Mutation::Unique => Ok(Box::new(
                                DynamicViewMut::<[u8; 32]>::from_storage_atomic_ref(
                                    all_storages
                                        .try_borrow()
                                        .map_err(error::GetStorage::AllStoragesBorrow)?,
                                    StorageId::Custom(intent.component.id),
                                )?,
                            )),
                        },
                    }
                },
            )
            .collect::<Result<_, _>>()?)
    }

    fn borrow_infos(&self, _: &mut Vec<(StorageId, Mutation)>) {}

    fn is_send_sync(&self) -> bool {
        true
    }
}

macro_rules! impl_system {
    ($(($type: ident, $index: tt))+) => {
        impl<'s, $($type: Borrow<'s>,)+ R, Func> System<'s, (), ($($type,)+), R> for Func where Func: FnOnce($($type),+) -> R {
            fn run(self, _: (), b: ($($type,)+)) -> R {
                (self)($(b.$index,)+)
            }
            fn try_borrow(
                &self,
                all_storages: &'s AtomicRefCell<AllStorages>,
                #[cfg(feature = "parallel")] thread_pool: &'s rayon::ThreadPool
            ) -> Result<($($type,)+), error::GetStorage> {
                #[cfg(feature = "parallel")]
                {
                    Ok(($($type::try_borrow(all_storages, thread_pool)?,)+))
                }
                #[cfg(not(feature = "parallel"))]
                {
                    Ok(($($type::try_borrow(all_storages)?,)+))
                }
            }
            fn borrow_infos(&self, infos: &mut Vec<(StorageId, Mutation)>) {
                $(
                    $type::borrow_infos(infos);
                )+
            }
            fn is_send_sync(&self) -> bool {
                $(
                    $type::is_send_sync()
                )&&+
            }
        }

        impl<'s, Data, $($type: Borrow<'s>,)+ R, Func> System<'s, (Data,), ($($type,)+), R> for Func where Func: FnOnce(Data, $($type,)+) -> R {
            fn run(self, (data,): (Data,), b: ($($type,)+)) -> R {
                (self)(data, $(b.$index,)+)
            }
            fn try_borrow(
                &self,
                all_storages: &'s AtomicRefCell<AllStorages>,
                #[cfg(feature = "parallel")] thread_pool: &'s rayon::ThreadPool
            ) -> Result<($($type,)+), error::GetStorage> {
                #[cfg(feature = "parallel")]
                {
                    Ok(($($type::try_borrow(all_storages, thread_pool)?,)+))
                }
                #[cfg(not(feature = "parallel"))]
                {
                    Ok(($($type::try_borrow(all_storages)?,)+))
                }
            }
            fn borrow_infos(&self,infos: &mut Vec<(StorageId, Mutation)>) {
                $(
                    $type::borrow_infos(infos);
                )+
            }
            fn is_send_sync(&self,) -> bool {
                $(
                    $type::is_send_sync()
                )&&+
            }
        }
    }
}

macro_rules! system {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_system![$(($type, $index))*];
        system![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_system![$(($type, $index))*];
    }
}

system![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
