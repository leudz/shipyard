use super::Nothing;
use crate::all_storages::AllStorages;
use crate::borrow::{AllStoragesBorrow, Borrow, IntoBorrow};
use crate::error;

/// Trait bound encompassing all functions that can be used as system.  
/// Same as `System` but for `AllStorages::run`.
///
/// `Data` is the external data passed to the system through `run_with_data`.
/// `Borrow` are the storages borrowed.
/// `Return` is the type returned by the system.
pub trait AllSystem<'s, Data, Borrow, Return> {
    #[allow(missing_docs)]
    fn run(self, data: Data, all_storages: &'s AllStorages) -> Result<Return, error::GetStorage>;
}

// Nothing has to be used and not () to not conflict where A = ()
impl<'s, Return, F> AllSystem<'s, (), Nothing, Return> for F
where
    F: FnOnce() -> Return,
{
    fn run(self, _: (), _: &'s AllStorages) -> Result<Return, error::GetStorage> {
        Ok((self)())
    }
}

// Nothing has to be used and not () to not conflict where A = ()
impl<'s, Data, Return, F> AllSystem<'s, (Data,), Nothing, Return> for F
where
    F: FnOnce(Data) -> Return,
{
    fn run(self, (data,): (Data,), _: &'s AllStorages) -> Result<Return, error::GetStorage> {
        Ok((self)(data))
    }
}

macro_rules! impl_all_system {
    ($(($type: ident, $index: tt))+) => {
        impl<'s, $($type: IntoBorrow,)+ Return, Func> AllSystem<'s, (), ($($type,)+), Return> for Func
        where
            Func: FnOnce($($type),+) -> Return
                + FnOnce($(<$type::Borrow as Borrow<'s>>::View),+) -> Return,
            $(
                $type::Borrow: AllStoragesBorrow<'s>,
            )+
        {
            fn run(
                self,
                _: (),
                all_storages: &'s AllStorages,
            ) -> Result<Return, error::GetStorage> {
                let current = all_storages.get_current();
                Ok(self($($type::Borrow::all_borrow(all_storages, None, current)?,)+))
            }
        }

        impl<'s, Data, $($type: IntoBorrow,)+ Return, Func> AllSystem<'s, (Data,), ($($type,)+), Return> for Func
        where
            Func: FnOnce(Data, $($type),+) -> Return
                + FnOnce(Data, $(<$type::Borrow as Borrow<'s>>::View),+) -> Return,
            $(
                $type::Borrow: AllStoragesBorrow<'s>,
            )+
        {
            fn run(
                self,
                (data,): (Data,),
                all_storages: &'s AllStorages,
            ) -> Result<Return, error::GetStorage> {
                let current = all_storages.get_current();
                Ok(self(data, $($type::Borrow::all_borrow(all_storages, None, current)?,)+))
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

all_system![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
