use super::m_loose_with_id::*;
use super::m_non_packed_with_id::*;
use super::m_tight_with_id::*;
use super::m_update_with_id::*;
use super::{AbstractMut, IntoAbstract};
use crate::entity::Key;

macro_rules! impl_iterators {
    (
        $number: literal
        $with_id: ident
        $tight_with_id: ident
        $loose_with_id: ident
        $non_packed_with_id: ident
        $update_with_id: ident
        $(($type: ident, $index: tt))+
    ) => {
        pub enum $with_id<$($type: IntoAbstract),+> {
            Tight($tight_with_id<$($type),+>),
            Loose($loose_with_id<$($type),+>),
            Update($update_with_id<$($type),+>),
            NonPacked($non_packed_with_id<$($type),+>),
        }

        impl<$($type: IntoAbstract),+> Iterator for $with_id<$($type),+> {
            type Item = (Key, $(<$type::AbsView as AbstractMut>::Out,)+);
            fn next(&mut self) -> Option<Self::Item> {
                match self {
                    $with_id::Tight(iter) => iter.next(),
                    $with_id::Loose(iter) => iter.next(),
                    $with_id::Update(iter) => iter.next(),
                    $with_id::NonPacked(iter) => iter.next(),
                }
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($with_id: ident)*; $with_id1: ident $($queue_with_id: ident)+;
        $($tight_with_id: ident)*; $tight_with_id1: ident $($queue_tight_with_id: ident)+;
        $($loose_with_id: ident)*; $loose_with_id1: ident $($queue_loose_with_id: ident)+;
        $($non_packed_with_id: ident)*; $non_packed_with_id1: ident $($queue_non_packed_with_id: ident)+;
        $($update_with_id: ident)*; $update_with_id1: ident $($queue_update_with_id: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $with_id1 $tight_with_id1 $loose_with_id1 $non_packed_with_id1 $update_with_id1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($with_id)* $with_id1; $($queue_with_id)+;
            $($tight_with_id)* $tight_with_id1; $($queue_tight_with_id)+;
            $($loose_with_id)* $loose_with_id1; $($queue_loose_with_id)+;
            $($non_packed_with_id)* $non_packed_with_id1; $($queue_non_packed_with_id)+;
            $($update_with_id)* $update_with_id1; $($queue_update_with_id)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($with_id: ident)*; $with_id1: ident;
        $($tight_with_id: ident)*; $tight_with_id1: ident;
        $($loose_with_id: ident)*; $loose_with_id1: ident;
        $($non_packed_with_id: ident)*; $non_packed_with_id1: ident;
        $($update_with_id: ident)*; $update_with_id1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $with_id1 $tight_with_id1 $loose_with_id1 $non_packed_with_id1 $update_with_id1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;WithId2 WithId3 WithId4 WithId5 WithId6 WithId7 WithId8 WithId9 WithId10;
    ;TightWithId2 TightWithId3 TightWithId4 TightWithId5 TightWithId6 TightWithId7 TightWithId8 TightWithId9 TightWithId10;
    ;LooseWithId2 LooseWithId3 LooseWithId4 LooseWithId5 LooseWithId6 LooseWithId7 LooseWithId8 LooseWithId9 LooseWithId10;
    ;NonPackedWithId2 NonPackedWithId3 NonPackedWithId4 NonPackedWithId5 NonPackedWithId6 NonPackedWithId7 NonPackedWithId8 NonPackedWithId9 NonPackedWithId10;
    ;UpdateWithId2 UpdateWithId3 UpdateWithId4 UpdateWithId5 UpdateWithId6 UpdateWithId7 UpdateWithId8 UpdateWithId9 UpdateWithId10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
