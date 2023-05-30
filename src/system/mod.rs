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
pub trait System<Data, Borrow> {
    /// The system return type
    type Return;

    #[allow(missing_docs)]
    fn run(self, data: Data, world: &World) -> Result<Self::Return, error::GetStorage>;
}

// `Nothing` has to be used and not `()` to not conflict where `A = ()`
impl<R, F> System<(), Nothing> for F
where
    F: FnOnce() -> R,
{
    type Return = R;

    fn run(self, _: (), _: &World) -> Result<R, error::GetStorage> {
        Ok((self)())
    }
}

// `Nothing` has to be used and not `()` to not conflict where `A = ()`
impl<Data, R, F> System<(Data,), Nothing> for F
where
    F: FnOnce(Data) -> R,
{
    type Return = R;

    fn run(self, (data,): (Data,), _: &World) -> Result<R, error::GetStorage> {
        Ok((self)(data))
    }
}

macro_rules! impl_system {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: WorldBorrow,)+ R, Func> System<(), ($($type,)+)> for Func
        where
            Func: FnOnce($($type),+) -> R
                + FnOnce($($type::WorldView<'_>),+) -> R
        {
            type Return = R;

            fn run(self, _: (), world: &World) -> Result<R, error::GetStorage> {
                let current = world.get_current();
                Ok((self)($($type::world_borrow(world, None, current)?,)+))
            }
        }

        impl<Data, $($type: WorldBorrow,)+ R, Func> System<(Data,), ($($type,)+)> for Func
        where
            Func: FnOnce(Data, $($type),+) -> R
                + FnOnce(Data, $($type::WorldView<'_>),+) -> R
        {
            type Return = R;

            fn run(self, (data,): (Data,), world: &World) -> Result<R, error::GetStorage> {
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
