#[cfg(feature = "parallel")]
use super::m_tight::*;
#[cfg(feature = "parallel")]
use super::{AbstractMut, IntoAbstract};
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{bridge, Consumer, ProducerCallback, UnindexedConsumer};
#[cfg(feature = "parallel")]
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

macro_rules! impl_iterators {
    (
        $number: literal
        $tight: ident
        $par_tight: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[doc = "Parallel tight iterator over"]
        #[doc = $number]
        #[doc = "components.\n Tight iterators are fast but are limited to components tightly packed together."]
        #[cfg(feature = "parallel")]
        pub struct $par_tight<$($type: IntoAbstract),+>(pub(super) $tight<$($type),+>);

        #[cfg(feature = "parallel")]
        impl<$($type: IntoAbstract),+> ParallelIterator for $par_tight<$($type),+>
        where
            $(<$type::AbsView as AbstractMut>::Out: Send),+
        {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn drive_unindexed<Cons>(self, consumer: Cons) -> Cons::Result
            where
                Cons: UnindexedConsumer<Self::Item>,
            {
                bridge(self, consumer)
            }
            fn opt_len(&self) -> Option<usize> {
                Some(self.len())
            }
        }

        #[cfg(feature = "parallel")]
        impl<$($type: IntoAbstract),+> IndexedParallelIterator for $par_tight<$($type),+>
        where
            $(<$type::AbsView as AbstractMut>::Out: Send),+
        {
            fn len(&self) -> usize {
                self.0.end - self.0.current
            }
            fn drive<Cons: Consumer<Self::Item>>(self, consumer: Cons) -> Cons::Result {
                bridge(self, consumer)
            }
            fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
                callback.callback(self.0)
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($tight: ident)*; $tight1: ident $($queue_tight: ident)+;
        $($par_tight: ident)*; $par_tight1: ident $($queue_par_tight: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $tight1 $par_tight1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($tight)* $tight1; $($queue_tight)+;
            $($par_tight)* $par_tight1; $($queue_par_tight)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($tight: ident)*; $tight1: ident;
        $($par_tight: ident)*; $par_tight1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $tight1 $par_tight1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Tight2 Tight3 Tight4 Tight5 Tight6 Tight7 Tight8 Tight9 Tight10;
    ;ParTight2 ParTight3 ParTight4 ParTight5 ParTight6 ParTight7 ParTight8 ParTight9 ParTight10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
