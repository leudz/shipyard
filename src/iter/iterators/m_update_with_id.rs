use super::m_update::*;
use super::{AbstractMut, IntoAbstract};
use crate::storage::Key;

macro_rules! impl_iterators {
    (
        $number: literal
        $update: ident
        $update_with_id: ident
        $(($type: ident, $index: tt))+
    ) => {
        pub struct $update_with_id<$($type: IntoAbstract),+>(pub(super) $update<$($type),+>);

        impl<$($type: IntoAbstract),+> Iterator for $update_with_id<$($type),+> {
            type Item = (Key, $(<<$type as IntoAbstract>::AbsView as AbstractMut>::Out),+);
            fn next(&mut self) -> Option<Self::Item> {
                self.0.next().map(|item| {
                    let id = self.0.last_id;
                    (id, $(item.$index),+)
                })
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($update: ident)*; $update1: ident $($queue_update: ident)+;
        $($update_with_id: ident)*; $update_with_id1: ident $($queue_update_with_id: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $update1 $update_with_id1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($update)* $update1; $($queue_update)+;
            $($update_with_id)* $update_with_id1; $($queue_update_with_id)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($update: ident)*; $update1: ident;
        $($update_with_id: ident)*; $update_with_id1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $update1 $update_with_id1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Update2 Update3 Update4 Update5 Update6 Update7 Update8 Update9 Update10;
    ;UpdateWithId2 UpdateWithId3 UpdateWithId4 UpdateWithId5 UpdateWithId6 UpdateWithId7 UpdateWithId8 UpdateWithId9 UpdateWithId10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
