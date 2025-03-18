use super::Nothing;
use crate::all_storages::AllStorages;
use crate::borrow::Borrow;
use crate::error;

/// Trait bound encompassing all functions that can be used as system.  
/// Same as `System` but for `AllStorages::run`.
///
/// `Data` is the external data passed to the system through `run_with_data`.
/// `Borrow` are the storages borrowed.
/// `Return` is the type returned by the system.
pub trait AllSystem<Data, Borrow> {
    /// The system return type
    type Return;

    #[allow(missing_docs)]
    fn run(self, data: Data, all_storages: &AllStorages)
        -> Result<Self::Return, error::GetStorage>;
}

// Nothing has to be used and not () to not conflict where A = ()
impl<R, F> AllSystem<(), Nothing> for F
where
    F: FnOnce() -> R,
{
    type Return = R;

    fn run(self, _: (), _: &AllStorages) -> Result<R, error::GetStorage> {
        Ok((self)())
    }
}

// Nothing has to be used and not () to not conflict where A = ()
impl<Data, R, F> AllSystem<(Data,), Nothing> for F
where
    F: FnOnce(Data) -> R,
{
    type Return = R;

    fn run(self, (data,): (Data,), _: &AllStorages) -> Result<R, error::GetStorage> {
        Ok((self)(data))
    }
}

macro_rules! impl_all_system {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: Borrow,)+ Ret, Func> AllSystem<(), ($($type,)+)> for Func
        where
            Func: FnOnce($($type),+) -> Ret
                + FnOnce($($type::View<'_>),+) -> Ret,
        {
            type Return = Ret;

            fn run(
                self,
                _: (),
                all_storages: &AllStorages,
            ) -> Result<Ret, error::GetStorage> {
                let current = all_storages.get_current();
                Ok(self($($type::borrow(all_storages, None, None, current)?,)+))
            }
        }

        impl<Data, $($type: Borrow,)+ Ret, Func> AllSystem<(Data,), ($($type,)+)> for Func
        where
            Func: FnOnce(Data, $($type),+) -> Ret
                + FnOnce(Data, $($type::View<'_>),+) -> Ret,
        {
            type Return = Ret;

            fn run(
                self,
                (data,): (Data,),
                all_storages: &AllStorages,
            ) -> Result<Ret, error::GetStorage> {
                let current = all_storages.get_current();
                Ok(self(data, $($type::borrow(all_storages, None, None, current)?,)+))
            }
        }
    }
}

macro_rules! all_system {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_all_system![$(($type, $index))*];
        all_system![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_all_system![$(($type, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
all_system![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
#[cfg(feature = "extended_tuple")]
all_system![
    (A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
    (K, 10) (L, 11) (M, 12) (N, 13) (O, 14) (P, 15) (Q, 16) (R, 17) (S, 18) (T, 19)
    (U, 20) (V, 21) (W, 22) (X, 23) (Y, 24) (Z, 25) (AA, 26) (BB, 27) (CC, 28) (DD, 29)
    (EE, 30) (FF, 31)
];
