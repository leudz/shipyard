mod all_storages;

pub use all_storages::AllSystem;

use crate::borrow::{BorrowInfo, WorldBorrow};
use crate::error;
use crate::scheduler::TypeInfo;
use crate::world::World;
use alloc::vec::Vec;

pub struct Nothing;

pub trait System<'s, Data, B, R> {
    fn run(self, data: Data, world: &'s World) -> Result<R, error::GetStorage>;
    fn borrow_info(info: &mut Vec<TypeInfo>);
}

// Nothing has to be used and not () to not conflict where A = ()
impl<'s, R, F> System<'s, (), Nothing, R> for F
where
    F: FnOnce() -> R,
{
    fn run(self, _: (), _: &'s World) -> Result<R, error::GetStorage> {
        Ok((self)())
    }

    fn borrow_info(_: &mut Vec<TypeInfo>) {}
}

// Nothing has to be used and not () to not conflict where A = ()
impl<'s, Data, R, F> System<'s, (Data,), Nothing, R> for F
where
    F: FnOnce(Data) -> R,
{
    fn run(self, (data,): (Data,), _: &'s World) -> Result<R, error::GetStorage> {
        Ok((self)(data))
    }

    fn borrow_info(_: &mut Vec<TypeInfo>) {}
}

macro_rules! impl_system {
    ($(($type: ident, $index: tt))+) => {
        impl<'s, $($type: WorldBorrow<'s> + BorrowInfo,)+ R, Func> System<'s, (), ($($type,)+), R> for Func where Func: FnOnce($($type),+) -> R {
            fn run(self, _: (), world: &'s World) -> Result<R, error::GetStorage> {
                Ok((self)($($type::borrow(world)?,)+))
            }

            fn borrow_info(info: &mut Vec<TypeInfo>) {
                $(
                    $type::borrow_info(info);
                )+
            }
        }

        impl<'s, Data, $($type: WorldBorrow<'s> + BorrowInfo,)+ R, Func> System<'s, (Data,), ($($type,)+), R> for Func where Func: FnOnce(Data, $($type,)+) -> R {
            fn run(self, (data,): (Data,), world: &'s World) -> Result<R, error::GetStorage> {
                Ok((self)(data, $($type::borrow(world)?,)+))
            }

            fn borrow_info(info: &mut Vec<TypeInfo>) {
                $(
                    $type::borrow_info(info);
                )+
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
