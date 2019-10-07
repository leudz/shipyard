use super::m_loose_filter::*;
use super::m_non_packed_filter::*;
use super::m_tight_filter::*;
use super::m_update_filter::*;
use super::{AbstractMut, IntoAbstract};

macro_rules! impl_iterators {
    (
        $number: literal
        $filter: ident
        $tight_filter: ident
        $loose_filter: ident
        $non_packed_filter: ident
        $update_filter: ident
        $(($type: ident, $index: tt))+
    ) => {
        pub enum $filter<$($type: IntoAbstract),+, P> {
            Tight($tight_filter<$($type),+, P>),
            Loose($loose_filter<$($type),+, P>),
            Update($update_filter<$($type),+, P>),
            NonPacked($non_packed_filter<$($type),+, P>),
        }

        impl<$($type: IntoAbstract),+, P: FnMut(&($(<$type::AbsView as AbstractMut>::Out,)+)) -> bool> Iterator for $filter<$($type),+, P> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn next(&mut self) -> Option<Self::Item> {
                match self {
                    $filter::Tight(iter) => iter.next(),
                    $filter::Loose(iter) => iter.next(),
                    $filter::Update(iter) => iter.next(),
                    $filter::NonPacked(iter) => iter.next(),
                }
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($filter: ident)*; $filter1: ident $($queue_filter: ident)+;
        $($tight_filter: ident)*; $tight_filter1: ident $($queue_tight_filter: ident)+;
        $($loose_filter: ident)*; $loose_filter1: ident $($queue_loose_filter: ident)+;
        $($non_packed_filter: ident)*; $non_packed_filter1: ident $($queue_non_packed_filter: ident)+;
        $($update_filter: ident)*; $update_filter1: ident $($queue_update_filter: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $filter1 $tight_filter1 $loose_filter1 $non_packed_filter1 $update_filter1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($filter)* $filter1; $($queue_filter)+;
            $($tight_filter)* $tight_filter1; $($queue_tight_filter)+;
            $($loose_filter)* $loose_filter1; $($queue_loose_filter)+;
            $($non_packed_filter)* $non_packed_filter1; $($queue_non_packed_filter)+;
            $($update_filter)* $update_filter1; $($queue_update_filter)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($filter: ident)*; $filter1: ident;
        $($tight_filter: ident)*; $tight_filter1: ident;
        $($loose_filter: ident)*; $loose_filter1: ident;
        $($non_packed_filter: ident)*; $non_packed_filter1: ident;
        $($update_filter: ident)*; $update_filter1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $filter1 $tight_filter1 $loose_filter1 $non_packed_filter1 $update_filter1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Filter2 Filter3 Filter4 Filter5 Filter6 Filter7 Filter8 Filter9 Filter10;
    ;TightFilter2 TightFilter3 TightFilter4 TightFilter5 TightFilter6 TightFilter7 TightFilter8 TightFilter9 TightFilter10;
    ;LooseFilter2 LooseFilter3 LooseFilter4 LooseFilter5 LooseFilter6 LooseFilter7 LooseFilter8 LooseFilter9 LooseFilter10;
    ;NonPackedFilter2 NonPackedFilter3 NonPackedFilter4 NonPackedFilter5 NonPackedFilter6 NonPackedFilter7 NonPackedFilter8 NonPackedFilter9 NonPackedFilter10;
    ;UpdateFilter2 UpdateFilter3 UpdateFilter4 UpdateFilter5 UpdateFilter6 UpdateFilter7 UpdateFilter8 UpdateFilter9 UpdateFilter10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
