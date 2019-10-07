use super::m_tight::*;
use super::{AbstractMut, IntoAbstract};

macro_rules! impl_iterators {
    (
        $number: literal
        $tight: ident
        $tight_filter: ident
        $(($type: ident, $index: tt))+
    ) => {
        pub struct $tight_filter<$($type: IntoAbstract),+, P> {
            pub(super) iter: $tight<$($type),+>,
            pub(super) pred: P,
        }

        impl<$($type: IntoAbstract),+, P: FnMut(&($(<$type::AbsView as AbstractMut>::Out,)+)) -> bool> Iterator for $tight_filter<$($type),+, P> {
            type Item = <$tight<$($type),+> as Iterator>::Item;
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
        $($tight: ident)*; $tight1: ident $($queue_tight: ident)+;
        $($tight_filter: ident)*; $tight_filter1: ident $($queue_tight_filter: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $tight1 $tight_filter1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($tight)* $tight1; $($queue_tight)+;
            $($tight_filter)* $tight_filter1; $($queue_tight_filter)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($tight: ident)*; $tight1: ident;
        $($tight_filter: ident)*; $tight_filter1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $tight1 $tight_filter1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Tight2 Tight3 Tight4 Tight5 Tight6 Tight7 Tight8 Tight9 Tight10;
    ;TightFilter2 TightFilter3 TightFilter4 TightFilter5 TightFilter6 TightFilter7 TightFilter8 TightFilter9 TightFilter10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
