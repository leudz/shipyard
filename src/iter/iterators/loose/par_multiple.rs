use super::{multiple::*, AbstractMut, ExactSizeShiperator, IntoAbstract};
use rayon::iter::plumbing::{bridge, Consumer, ProducerCallback, UnindexedConsumer};
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

macro_rules! impl_iterators {
    (
        $number: literal
        $loose: ident
        $seq: ident
        $(($type: ident, $index: tt))+
    ) => {
        pub struct $loose<$($type: IntoAbstract),+>($seq<$($type),+>);

        impl<$($type: IntoAbstract),+> From<$seq<$($type),+>> for $loose<$($type),+> {
            fn from(seq: $seq<$($type),+>) -> Self {
                $loose(seq)
            }
        }

        impl<$($type: IntoAbstract),+> ParallelIterator for $loose<$($type),+>
        where $($type::AbsView: Clone + Send,)+ $(<$type::AbsView as AbstractMut>::Out: Send),+ {
            type Item = ($(<$type::AbsView as AbstractMut>::Out),+);
            fn drive_unindexed<Con>(self, consumer: Con) -> Con::Result where Con: UnindexedConsumer<Self::Item> {
                bridge(self, consumer)
            }
            fn opt_len(&self) -> Option<usize> {
                Some(self.len())
            }
        }

        impl<$($type: IntoAbstract),+> IndexedParallelIterator for $loose<$($type),+>
        where $($type::AbsView: Clone + Send,)+ $(<$type::AbsView as AbstractMut>::Out: Send),+ {
            fn len(&self) -> usize {
                self.0.len()
            }
            fn drive<Con>(self, consumer: Con) -> Con::Result where Con: Consumer<Self::Item> {
                bridge(self, consumer)
            }
            fn with_producer<CB>(self, callback: CB) -> CB::Output where CB: ProducerCallback<Self::Item> {
                callback.callback(self.0)
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($loose: ident)*; $loose1: ident $($queue_loose: ident)+;
        $($seq: ident)*; $seq1: ident $($queue_seq: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $loose1 $seq1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($loose)* $loose1; $($queue_loose)+;
            $($seq)* $seq1; $($queue_seq)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($loose: ident)*; $loose1: ident;
        $($seq: ident)*; $seq1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $loose1 $seq1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;ParLoose2 ParLoose3 ParLoose4 ParLoose5 ParLoose6 ParLoose7 ParLoose8 ParLoose9 ParLoose10;
    ;Loose2 Loose3 Loose4 Loose5 Loose6 Loose7 Loose8 Loose9 Loose10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
