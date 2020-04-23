mod all_storages;

pub use all_storages::AllSystem;

use crate::atomic_refcell::AtomicRefCell;
use crate::borrow::Borrow;
use crate::borrow::Mutation;
use crate::error;
use crate::storage::AllStorages;
use alloc::vec::Vec;
use core::any::TypeId;

pub struct Nothing;

pub trait System<'s, B, R> {
    fn run(self, b: B) -> R;
    fn try_borrow(
        all_storages: &'s AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] thread_pool: &'s rayon::ThreadPool,
    ) -> Result<B, error::GetStorage>;

    fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>);

    fn is_send_sync() -> bool;
}

// Nothing has to be used and not () to not conflict where A = ()
impl<'s, R, F> System<'s, Nothing, R> for F
where
    F: FnOnce() -> R,
{
    fn run(self, _: Nothing) -> R {
        (self)()
    }
    fn try_borrow(
        _: &'s AtomicRefCell<AllStorages>,
        #[cfg(feature = "parallel")] _: &'s rayon::ThreadPool,
    ) -> Result<Nothing, error::GetStorage> {
        Ok(Nothing)
    }

    fn borrow_infos(_: &mut Vec<(TypeId, Mutation)>) {}

    fn is_send_sync() -> bool {
        true
    }
}

macro_rules! impl_system {
    ($(($type: ident, $index: tt))+) => {
        impl<'s, $($type: Borrow<'s>,)+ R, Func> System<'s, ($($type,)+), R> for Func where Func: FnOnce($($type),+) -> R {
            fn run(self, b: ($($type,)+)) -> R {
                (self)($(b.$index,)+)
            }
            fn try_borrow(
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
            fn borrow_infos(infos: &mut Vec<(TypeId, Mutation)>) {
                $(
                    $type::borrow_infos(infos);
                )+
            }
            fn is_send_sync() -> bool {
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
