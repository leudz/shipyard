use super::m_loose::*;
use super::{AbstractMut, IntoAbstract};

macro_rules! impl_iterators {
    (
        $number: literal
        $loose: ident
        $loose_filter: ident
        $(($type: ident, $index: tt))+
    ) => {
        pub struct $loose_filter<$($type: IntoAbstract),+, P> {
            pub(super) iter: $loose<$($type),+>,
            pub(super) pred: P,
        }

        impl<$($type: IntoAbstract),+, P: FnMut(&($(<$type::AbsView as AbstractMut>::Out,)+)) -> bool> Iterator for $loose_filter<$($type),+, P> {
            type Item = <$loose<$($type),+> as Iterator>::Item;
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
        $($loose: ident)*; $loose1: ident $($queue_loose: ident)+;
        $($loose_filter: ident)*; $loose_filter1: ident $($queue_loose_filter: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $loose1 $loose_filter1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($loose)* $loose1; $($queue_loose)+;
            $($loose_filter)* $loose_filter1; $($queue_loose_filter)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($loose: ident)*; $loose1: ident;
        $($loose_filter: ident)*; $loose_filter1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $loose1 $loose_filter1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Loose2 Loose3 Loose4 Loose5 Loose6 Loose7 Loose8 Loose9 Loose10;
    ;LooseFilter2 LooseFilter3 LooseFilter4 LooseFilter5 LooseFilter6 LooseFilter7 LooseFilter8 LooseFilter9 LooseFilter10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
