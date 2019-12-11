use super::m_tight::*;
use super::{AbstractMut, IntoAbstract};
use crate::storage::Key;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::Producer;

macro_rules! impl_iterators {
    (
        $number: literal
        $tight: ident
        $tight_with_id: ident
        $(($type: ident, $index: tt))+
    ) => {
        pub struct $tight_with_id<$($type: IntoAbstract),+>(pub(super) $tight<$($type),+>);

        impl<$($type: IntoAbstract),+> Iterator for $tight_with_id<$($type),+> {
            type Item = (Key, $(<$type::AbsView as AbstractMut>::Out,)+);
            fn next(&mut self) -> Option<Self::Item> {
                self.0.next().map(|item| {
                    let id = unsafe { self.0.data.0.id_at(self.0.current - 1) };
                    (id, $(item.$index),+)
                })
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                (self.len(), Some(self.len()))
            }
        }

        impl<$($type: IntoAbstract),+> DoubleEndedIterator for $tight_with_id<$($type),+> {
            fn next_back(&mut self) -> Option<Self::Item> {
                self.0.next_back().map(|item| {
                    let id = unsafe { self.0.data.0.id_at(self.0.end) };
                    (id, $(item.$index),+)
                })
            }
        }

        impl<$($type: IntoAbstract),+> ExactSizeIterator for $tight_with_id<$($type),+> {
            fn len(&self) -> usize {
                self.0.len()
            }
        }

        #[cfg(feature = "parallel")]
        impl<$($type: IntoAbstract),+> Producer for $tight_with_id<$($type),+>
        where
            $(<$type::AbsView as AbstractMut>::Out: Send),+
        {
            type Item = (Key, $(<$type::AbsView as AbstractMut>::Out,)+);
            type IntoIter = Self;
            fn into_iter(self) -> Self::IntoIter {
                self
            }
            fn split_at(mut self, index: usize) -> (Self, Self) {
                let (left, right) = self.0.split_at(index);
                self.0 = left;
                let clone = $tight_with_id(right);
                (self, clone)
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($tight: ident)*; $tight1: ident $($queue_tight: ident)+;
        $($tight_with_id: ident)*; $tight_with_id1: ident $($queue_tight_with_id: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $tight1 $tight_with_id1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($tight)* $tight1; $($queue_tight)+;
            $($tight_with_id)* $tight_with_id1; $($queue_tight_with_id)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($tight: ident)*; $tight1: ident;
        $($tight_with_id: ident)*; $tight_with_id1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $tight1 $tight_with_id1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Tight2 Tight3 Tight4 Tight5 Tight6 Tight7 Tight8 Tight9 Tight10;
    ;TightWithId2 TightWithId3 TightWithId4 TightWithId5 TightWithId6 TightWithId7 TightWithId8 TightWithId9 TightWithId10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
