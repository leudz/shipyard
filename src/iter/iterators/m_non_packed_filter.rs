use super::m_non_packed::*;
use super::{AbstractMut, IntoAbstract};

macro_rules! impl_iterators {
    (
        $number: literal
        $non_packed: ident
        $non_packed_filter: ident
        $(($type: ident, $index: tt))+
    ) => {
        pub struct $non_packed_filter<$($type: IntoAbstract),+, P> {
            pub(super) iter: $non_packed<$($type),+>,
            pub(super) pred: P,
        }

        impl<$($type: IntoAbstract),+, P: FnMut(&($(<$type::AbsView as AbstractMut>::Out,)+)) -> bool> Iterator for $non_packed_filter<$($type),+, P> {
            type Item = <$non_packed<$($type),+> as Iterator>::Item;
            fn next(&mut self) -> Option<Self::Item> {
                for item in &mut self.iter {
                    if (self.pred)(&item) {
                        return Some(item)
                    }
                }
                None
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($non_packed: ident)*; $non_packed1: ident $($queue_non_packed: ident)+;
        $($non_packed_filter: ident)*; $non_packed_filter1: ident $($queue_non_packed_filter: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $non_packed1 $non_packed_filter1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($non_packed)* $non_packed1; $($queue_non_packed)+;
            $($non_packed_filter)* $non_packed_filter1; $($queue_non_packed_filter)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($non_packed: ident)*; $non_packed1: ident;
        $($non_packed_filter: ident)*; $non_packed_filter1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $non_packed1 $non_packed_filter1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;NonPacked2 NonPacked3 NonPacked4 NonPacked5 NonPacked6 NonPacked7 NonPacked8 NonPacked9 NonPacked10;
    ;NonPackedFilter2 NonPackedFilter3 NonPackedFilter4 NonPackedFilter5 NonPackedFilter6 NonPackedFilter7 NonPackedFilter8 NonPackedFilter9 NonPackedFilter10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
