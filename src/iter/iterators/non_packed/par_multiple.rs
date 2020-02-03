use super::super::update::*;
use super::{multiple::*, AbstractMut, IntoAbstract};
use rayon::iter::plumbing::{bridge_unindexed, UnindexedConsumer};
use rayon::iter::ParallelIterator;

macro_rules! impl_iterators {
    (
        $number: literal
        $non_packed: ident
        $seq: ident
        $update: ident
        $(($type: ident, $index: tt))+
    ) => {
        pub struct $non_packed<$($type: IntoAbstract),+>($seq<$($type),+>);

        impl<$($type: IntoAbstract),+> From<$seq<$($type),+>> for $non_packed<$($type),+> {
            fn from(seq: $seq<$($type),+>) -> Self {
                $non_packed(seq)
            }
        }

        impl<$($type: IntoAbstract),+> From<$update<$($type),+>> for $non_packed<$($type),+> {
            fn from(update: $update<$($type),+>) -> Self {
                $non_packed(update.into())
            }
        }

        impl<$($type: IntoAbstract),+> ParallelIterator for $non_packed<$($type),+>
        where $($type::AbsView: Clone + Send,)+ $(<$type::AbsView as AbstractMut>::Out: Send),+
        {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn drive_unindexed<Con>(mut self, consumer: Con) -> Con::Result
            where Con: UnindexedConsumer<Self::Item> {
                $(
                    unsafe {self.0.data.$index.flag_all()};
                )+
                bridge_unindexed(self.0, consumer)
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($loose: ident)*; $loose1: ident $($queue_loose: ident)+;
        $($seq: ident)*; $seq1: ident $($queue_seq: ident)+;
        $($update: ident)*; $update1: ident $($queue_update: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $loose1 $seq1 $update1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($loose)* $loose1; $($queue_loose)+;
            $($seq)* $seq1; $($queue_seq)+;
            $($update)* $update1; $($queue_update)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($loose: ident)*; $loose1: ident;
        $($seq: ident)*; $seq1: ident;
        $($update: ident)*; $update1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $loose1 $seq1 $update1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;ParNonPacked2 ParNonPacked3 ParNonPacked4 ParNonPacked5 ParNonPacked6 ParNonPacked7 ParNonPacked8 ParNonPacked9 ParNonPacked10;
    ;NonPacked2 NonPacked3 NonPacked4 NonPacked5 NonPacked6 NonPacked7 NonPacked8 NonPacked9 NonPacked10;
    ;Update2 Update3 Update4 Update5 Update6 Update7 Update8 Update9 Update10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
