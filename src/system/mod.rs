mod all_storages;

pub use all_storages::AllSystem;

use crate::borrow::WorldBorrow;
use crate::error;
use crate::world::World;

/// Used instead of `()` to not conflict where `A = ()`
pub struct Nothing;

/// Trait bound encompassing all functions that can be used as system.
///
/// `Data` is the external data passed to the system through `run_with_data`.
/// `Borrow` are the storages borrowed.
/// `Return` is the type returned by the system.
pub trait System<Data, Borrow, Return> {
    #[allow(missing_docs)]
    fn run(self, data: Data, world: &World) -> Result<Return, error::GetStorage>;
}

// `Nothing` has to be used and not `()` to not conflict where `A = ()`
impl<Return, F> System<(), Nothing, Return> for F
where
    F: FnOnce() -> Return,
{
    fn run(self, _: (), _: &World) -> Result<Return, error::GetStorage> {
        Ok((self)())
    }
}

// `Nothing` has to be used and not `()` to not conflict where `A = ()`
impl<Data, Return, F> System<(Data,), Nothing, Return> for F
where
    F: FnOnce(Data) -> Return,
{
    fn run(self, (data,): (Data,), _: &World) -> Result<Return, error::GetStorage> {
        Ok((self)(data))
    }
}

macro_rules! impl_system {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: WorldBorrow,)+ Return, Func> System<(), ($($type,)+), Return> for Func
        where
            Func: FnOnce($($type),+) -> Return
                + FnOnce($($type::WorldView<'_>),+) -> Return
        {
            fn run(self, _: (), world: &World) -> Result<Return, error::GetStorage> {
                let current = world.get_current();
                Ok((self)($($type::world_borrow(world, None, current)?,)+))
            }
        }

        impl<Data, $($type: WorldBorrow,)+ Return, Func> System<(Data,), ($($type,)+), Return> for Func
        where
            Func: FnOnce(Data, $($type),+) -> Return
                + FnOnce(Data, $($type::WorldView<'_>),+) -> Return
        {
            fn run(self, (data,): (Data,), world: &World) -> Result<Return, error::GetStorage> {
                let current = world.get_current();
                Ok((self)(data, $($type::world_borrow(world, None, current)?,)+))
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
