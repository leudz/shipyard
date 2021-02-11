mod all_storages;

pub use all_storages::AllSystem;

use crate::borrow::{Borrow, IntoBorrow};
use crate::error;
use crate::world::World;

pub struct Nothing;

pub trait System<'s, Data, B, R> {
    fn run(self, data: Data, world: &'s World) -> Result<R, error::GetStorage>;
}

// Nothing has to be used and not () to not conflict where A = ()
impl<'s, R, F> System<'s, (), Nothing, R> for F
where
    F: FnOnce() -> R,
{
    fn run(self, _: (), _: &'s World) -> Result<R, error::GetStorage> {
        Ok((self)())
    }
}

// Nothing has to be used and not () to not conflict where A = ()
impl<'s, Data, R, F> System<'s, (Data,), Nothing, R> for F
where
    F: FnOnce(Data) -> R,
{
    fn run(self, (data,): (Data,), _: &'s World) -> Result<R, error::GetStorage> {
        Ok((self)(data))
    }
}

macro_rules! impl_system {
    ($(($type: ident, $index: tt))+) => {
        impl<'s, $($type: IntoBorrow,)+ R, Func> System<'s, (), ($($type,)+), R> for Func
        where
            Func: FnOnce($($type),+) -> R
                + FnOnce($(<$type::Borrow as Borrow<'s>>::View),+) -> R
        {
            fn run(self, _: (), world: &'s World) -> Result<R, error::GetStorage> {
                Ok((self)($($type::Borrow::borrow(world)?,)+))
            }
        }

        impl<'s, Data, $($type: IntoBorrow,)+ R, Func> System<'s, (Data,), ($($type,)+), R> for Func
        where
            Func: FnOnce(Data, $($type),+) -> R
                + FnOnce(Data, $(<$type::Borrow as Borrow<'s>>::View),+) -> R
        {
            fn run(self, (data,): (Data,), world: &'s World) -> Result<R, error::GetStorage> {
                Ok((self)(data, $($type::Borrow::borrow(world)?,)+))
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
