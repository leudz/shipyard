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
        impl<$($type: WorldBorrow,)+ Ret, Func> System<(), ($($type,)+)> for Func
        where
            Func: FnOnce($($type),+) -> Ret
                + FnOnce($($type::WorldView<'_>),+) -> Ret
        {
            type Return = Ret;

            fn run(self, _: (), world: &World) -> Result<Ret, error::GetStorage> {
                let current = world.get_current();
                Ok((self)($($type::world_borrow(world, None, current)?,)+))
            }
        }

        impl<Data, $($type: WorldBorrow,)+ Ret, Func> System<(Data,), ($($type,)+)> for Func
        where
            Func: FnOnce(Data, $($type),+) -> Ret
                + FnOnce(Data, $($type::WorldView<'_>),+) -> Ret
        {
            type Return = Ret;

            fn run(self, (data,): (Data,), world: &World) -> Result<Ret, error::GetStorage> {
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

#[cfg(not(feature = "extended_tuple"))]
system![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
#[cfg(feature = "extended_tuple")]
system![
    (A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
    (K, 10) (L, 11) (M, 12) (N, 13) (O, 14) (P, 15) (Q, 16) (R, 17) (S, 18) (T, 19)
    (U, 20) (V, 21) (W, 22) (X, 23) (Y, 24) (Z, 25) (AA, 26) (BB, 27) (CC, 28) (DD, 29)
    (EE, 30) (FF, 31)
];
