use super::m_loose::*;
use super::{AbstractMut, IntoAbstract};
use crate::entity::Key;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::Producer;

macro_rules! impl_iterators {
    (
        $number: literal
        $loose: ident
        $loose_with_id: ident
        $(($type: ident, $index: tt))+
    ) => {
        pub struct $loose_with_id<$($type: IntoAbstract),+>(pub(super) $loose<$($type),+>);

        impl<$($type: IntoAbstract),+> Iterator for $loose_with_id<$($type,)+> {
            type Item = (Key, ($(<$type::AbsView as AbstractMut>::Out,)+));
            fn next(&mut self) -> Option<Self::Item> {
                self.0.next().map(|item| {
                    let id = unsafe { self.0.data.0.id_at(self.0.current - 1) };
                    (id, item)
                })
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.0.size_hint()
            }
        }

        impl<$($type: IntoAbstract),+> DoubleEndedIterator for $loose_with_id<$($type,)+> {
            fn next_back(&mut self) -> Option<Self::Item> {
                self.0.next_back().map(|item| {
                    let id = unsafe { self.0.data.0.id_at(self.0.end) };
                    (id, item)
                })
            }
        }

        impl<$($type: IntoAbstract),+> ExactSizeIterator for $loose_with_id<$($type),+> {
            fn len(&self) -> usize {
                self.0.len()
            }
        }

        #[cfg(feature = "parallel")]
        impl<$($type: IntoAbstract),+> Producer for $loose_with_id<$($type),+>
        where
            $(<$type::AbsView as AbstractMut>::Out: Send),+
        {
            type Item = (Key, ($(<$type::AbsView as AbstractMut>::Out,)+));
            type IntoIter = Self;
            fn into_iter(self) -> Self::IntoIter {
                self
            }
            fn split_at(mut self, index: usize) -> (Self, Self) {
                let (left, right) = self.0.split_at(index);
                self.0 = left;
                let clone = $loose_with_id(right);
                (self, clone)
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($loose: ident)*; $loose1: ident $($queue_loose: ident)+;
        $($loose_with_id: ident)*; $loose_with_id1: ident $($queue_loose_with_id: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $loose1 $loose_with_id1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($loose)* $loose1; $($queue_loose)+;
            $($loose_with_id)* $loose_with_id1; $($queue_loose_with_id)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($loose: ident)*; $loose1: ident;
        $($loose_with_id: ident)*; $loose_with_id1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $loose1 $loose_with_id1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Loose2 Loose3 Loose4 Loose5 Loose6 Loose7 Loose8 Loose9 Loose10;
    ;LooseWithId2 LooseWithId3 LooseWithId4 LooseWithId5 LooseWithId6 LooseWithId7 LooseWithId8 LooseWithId9 LooseWithId10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
