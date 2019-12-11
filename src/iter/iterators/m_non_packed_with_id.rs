use super::m_non_packed::*;
use super::{AbstractMut, IntoAbstract};
use crate::storage::Key;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{Folder, UnindexedProducer};

macro_rules! impl_iterators {
    (
        $number: literal
        $non_packed: ident
        $non_packed_with_id: ident
        $(($type: ident, $index: tt))+
    ) => {
        pub struct $non_packed_with_id<$($type: IntoAbstract),+>(pub(super) $non_packed<$($type),+>);

        impl<$($type: IntoAbstract),+> Iterator for $non_packed_with_id<$($type),+> {
            type Item = (Key, $(<$type::AbsView as AbstractMut>::Out,)+);
            fn next(&mut self) -> Option<Self::Item> {
                self.0.next().map(|item| {
                    let id = unsafe { self.0.data.0.id_at(self.0.current - 1) };
                    (id, $(item.$index),+)
                })
            }
        }

        impl<$($type: IntoAbstract),+> $non_packed_with_id<$($type),+> {
            #[cfg(feature = "parallel")]
            fn clone(&self) -> Self {
                $non_packed_with_id(self.0.clone())
            }
        }

        #[cfg(feature = "parallel")]
        impl<$($type: IntoAbstract),+> UnindexedProducer for $non_packed_with_id<$($type),+>
        where
            $(<$type::AbsView as AbstractMut>::Out: Send),+
        {
            type Item = (Key, $(<$type::AbsView as AbstractMut>::Out,)+);
            fn split(mut self) -> (Self, Option<Self>) {
                let len = self.0.end - self.0.current;
                if len >= 2 {
                    let mut clone = self.clone();
                    clone.0.current += len / 2;
                    self.0.end = clone.0.current;
                    (self, Some(clone))
                } else {
                    (self, None)
                }
            }
            fn fold_with<Fold>(self, folder: Fold) -> Fold
            where Fold: Folder<Self::Item> {
                folder.consume_iter(self)
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($non_packed: ident)*; $non_packed1: ident $($queue_non_packed: ident)+;
        $($non_packed_with_id: ident)*; $non_packed_with_id1: ident $($queue_non_packed_with_id: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $non_packed1 $non_packed_with_id1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($non_packed)* $non_packed1; $($queue_non_packed)+;
            $($non_packed_with_id)* $non_packed_with_id1; $($queue_non_packed_with_id)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($non_packed: ident)*; $non_packed1: ident;
        $($non_packed_with_id: ident)*; $non_packed_with_id1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $non_packed1 $non_packed_with_id1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;NonPacked2 NonPacked3 NonPacked4 NonPacked5 NonPacked6 NonPacked7 NonPacked8 NonPacked9 NonPacked10;
    ;NonPackedWithId2 NonPackedWithId3 NonPackedWithId4 NonPackedWithId5 NonPackedWithId6 NonPackedWithId7 NonPackedWithId8 NonPackedWithId9 NonPackedWithId10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
