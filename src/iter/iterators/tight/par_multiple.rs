use super::{multiple::*, AbstractMut, ExactSizeShiperator, IntoAbstract};
use rayon::iter::plumbing::{bridge, Consumer, ProducerCallback, UnindexedConsumer};
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

macro_rules! impl_iterators {
    (
        $number: literal
        $tight: ident
        $seq: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[doc = "Tight parallel iterator over"]
        #[doc = $number]
        #[doc = "components.  
Tight iterators are fast but are limited to components tightly packed together."]
        #[cfg_attr(docsrs, doc(cfg(feature = "parallel")))]
        pub struct $tight<$($type: IntoAbstract),+>($seq<$($type),+>);

        impl<$($type: IntoAbstract),+> From<$seq<$($type),+>> for $tight<$($type),+> {
            fn from(seq: $seq<$($type),+>) -> Self {
                $tight(seq)
            }
        }

        impl<$($type: IntoAbstract),+> ParallelIterator for $tight<$($type),+>
        where $($type::AbsView: Clone + Send,)+ $(<$type::AbsView as AbstractMut>::Out: Send),+ {
            type Item = ($(<$type::AbsView as AbstractMut>::Out),+);
            fn drive_unindexed<Con>(self, consumer: Con) -> Con::Result where Con: UnindexedConsumer<Self::Item> {
                bridge(self, consumer)
            }
            fn opt_len(&self) -> Option<usize> {
                Some(self.len())
            }
        }

        impl<$($type: IntoAbstract),+> IndexedParallelIterator for $tight<$($type),+>
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
        $($tight: ident)*; $tight1: ident $($queue_tight: ident)+;
        $($chunk: ident)*; $chunk1: ident $($queue_chunk: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $tight1 $chunk1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($tight)* $tight1; $($queue_tight)+;
            $($chunk)* $chunk1; $($queue_chunk)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($tight: ident)*; $tight1: ident;
        $($chunk: ident)*; $chunk1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $tight1 $chunk1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;ParTight2 ParTight3 ParTight4 ParTight5 ParTight6 ParTight7 ParTight8 ParTight9 ParTight10;
    ;Tight2 Tight3 Tight4 Tight5 Tight6 Tight7 Tight8 Tight9 Tight10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
