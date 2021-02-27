mod all_storages;

pub use all_storages::AllSystem;

use crate::borrow::{Borrow, IntoBorrow};
use crate::error;
use crate::world::World;

/// Used instead of `()` to not conflict where `A = ()`
pub struct Nothing;

/// Trait bound encompassing all functions that can be used as system.
///
/// `Data` is the external data passed to the system through `run_with_data`.
/// `Borrow` are the storages borrowed.
/// `Return` is the type returned by the system.
pub trait System<'s, Data, Borrow, Return> {
    #[allow(missing_docs)]
    fn run(self, data: Data, world: &'s World) -> Result<Return, error::GetStorage>;
}

// `Nothing` has to be used and not `()` to not conflict where `A = ()`
impl<'s, Return, F> System<'s, (), Nothing, Return> for F
where
    F: FnOnce() -> Return,
{
    fn run(self, _: (), _: &'s World) -> Result<Return, error::GetStorage> {
        Ok((self)())
    }
}

// `Nothing` has to be used and not `()` to not conflict where `A = ()`
impl<'s, Data, Return, F> System<'s, (Data,), Nothing, Return> for F
where
    F: FnOnce(Data) -> Return,
{
    fn run(self, (data,): (Data,), _: &'s World) -> Result<Return, error::GetStorage> {
        Ok((self)(data))
    }
}

macro_rules! impl_system {
    ($(($type: ident, $index: tt))+) => {
        impl<'s, $($type: IntoBorrow,)+ Return, Func> System<'s, (), ($($type,)+), Return> for Func
        where
            Func: FnOnce($($type),+) -> Return
                + FnOnce($(<$type::Borrow as Borrow<'s>>::View),+) -> Return
        {
            fn run(self, _: (), world: &'s World) -> Result<Return, error::GetStorage> {
                Ok((self)($($type::Borrow::borrow(world)?,)+))
            }
        }

        impl<'s, Data, $($type: IntoBorrow,)+ Return, Func> System<'s, (Data,), ($($type,)+), Return> for Func
        where
            Func: FnOnce(Data, $($type),+) -> Return
                + FnOnce(Data, $(<$type::Borrow as Borrow<'s>>::View),+) -> Return
        {
            fn run(self, (data,): (Data,), world: &'s World) -> Result<Return, error::GetStorage> {
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
