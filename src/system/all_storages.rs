use super::Nothing;
use crate::all_storages::AllStorages;
use crate::borrow::{AllStoragesBorrow, Borrow, IntoBorrow};
use crate::error;

pub trait AllSystem<'s, Data, B, R> {
    fn run(self, data: Data, all_storages: &'s AllStorages) -> Result<R, error::GetStorage>;
}

// Nothing has to be used and not () to not conflict where A = ()
impl<'s, R, F> AllSystem<'s, (), Nothing, R> for F
where
    F: FnOnce() -> R,
{
    fn run(self, _: (), _: &'s AllStorages) -> Result<R, error::GetStorage> {
        Ok((self)())
    }
}

// Nothing has to be used and not () to not conflict where A = ()
impl<'s, Data, R, F> AllSystem<'s, (Data,), Nothing, R> for F
where
    F: FnOnce(Data) -> R,
{
    fn run(self, (data,): (Data,), _: &'s AllStorages) -> Result<R, error::GetStorage> {
        Ok((self)(data))
    }
}

macro_rules! impl_all_system {
    ($(($type: ident, $index: tt))+) => {
        impl<'s, $($type: IntoBorrow,)+ R, Func> AllSystem<'s, (), ($($type,)+), R> for Func
        where
            Func: FnOnce($($type),+) -> R
                + FnOnce($(<$type::Borrow as Borrow<'s>>::View),+) -> R,
            $(
                $type::Borrow: AllStoragesBorrow<'s>,
            )+
        {
            fn run(
                self,
                _: (),
                all_storages: &'s AllStorages,
            ) -> Result<R, error::GetStorage> {
                    Ok(self($($type::Borrow::all_borrow(all_storages)?,)+))
            }
        }

        impl<'s, Data, $($type: IntoBorrow,)+ R, Func> AllSystem<'s, (Data,), ($($type,)+), R> for Func
        where
            Func: FnOnce(Data, $($type),+) -> R
                + FnOnce(Data, $(<$type::Borrow as Borrow<'s>>::View),+) -> R,
            $(
                $type::Borrow: AllStoragesBorrow<'s>,
            )+
        {
            fn run(
                self,
                (data,): (Data,),
                all_storages: &'s AllStorages,
            ) -> Result<R, error::GetStorage> {
                    Ok(self(data, $($type::Borrow::all_borrow(all_storages)?,)+))
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
