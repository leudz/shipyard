#[cfg(feature = "parallel")]
use super::m_loose::*;
#[cfg(feature = "parallel")]
use super::{AbstractMut, IntoAbstract};
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{bridge, Consumer, ProducerCallback, UnindexedConsumer};
#[cfg(feature = "parallel")]
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

macro_rules! impl_iterators {
    (
        $number: literal
        $loose: ident
        $par_loose: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[doc = "Parallel loose iterator over"]
        #[doc = $number]
        #[doc = "components.\n"]
        #[cfg(feature = "parallel")]
        pub struct $par_loose<$($type: IntoAbstract),+>(pub(super) $loose<$($type),+>);

        #[cfg(feature = "parallel")]
        impl<$($type: IntoAbstract),+> ParallelIterator for $par_loose<$($type),+>
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
        impl<$($type: IntoAbstract),+> IndexedParallelIterator for $par_loose<$($type),+>
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
        $($loose: ident)*; $loose1: ident $($queue_loose: ident)+;
        $($par_loose: ident)*; $par_loose1: ident $($queue_par_loose: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $loose1 $par_loose1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($loose)* $loose1; $($queue_loose)+;
            $($par_loose)* $par_loose1; $($queue_par_loose)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($loose: ident)*; $loose1: ident;
        $($par_loose: ident)*; $par_loose1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $loose1 $par_loose1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Loose2 Loose3 Loose4 Loose5 Loose6 Loose7 Loose8 Loose9 Loose10;
    ;ParLoose2 ParLoose3 ParLoose4 ParLoose5 ParLoose6 ParLoose7 ParLoose8 ParLoose9 ParLoose10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
