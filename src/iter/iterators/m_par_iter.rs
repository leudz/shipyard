#[cfg(feature = "parallel")]
use super::m_par_loose::*;
#[cfg(feature = "parallel")]
use super::m_par_non_packed::*;
#[cfg(feature = "parallel")]
use super::m_par_tight::*;
#[cfg(feature = "parallel")]
use super::m_par_update::*;
#[cfg(feature = "parallel")]
use super::{AbstractMut, IntoAbstract};
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{bridge, UnindexedConsumer};
#[cfg(feature = "parallel")]
use rayon::iter::ParallelIterator;

macro_rules! impl_iterators {
    (
        $number: literal
        $par_iter: ident
        $par_tight: ident
        $par_loose: ident
        $par_non_packed: ident
        $par_update: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[doc = "Parallel iterator over"]
        #[doc = $number]
        #[doc = "components.\n This enum allows to abstract away what kind of iterator you really get. That doesn't mean the performance will suffer, the compiler will (almost)
        always optimize it away."]
        #[cfg(feature = "parallel")]
        pub enum $par_iter<$($type: IntoAbstract),+> {
            Tight($par_tight<$($type),+>),
            Loose($par_loose<$($type),+>),
            Update($par_update<$($type),+>),
            NonPacked($par_non_packed<$($type),+>),
        }

        #[cfg(feature = "parallel")]
        impl<$($type: IntoAbstract),+> ParallelIterator for $par_iter<$($type),+>
        where $(<$type::AbsView as AbstractMut>::Out: Send),+ {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn drive_unindexed<Cons>(self, consumer: Cons) -> Cons::Result
            where
                Cons: UnindexedConsumer<Self::Item>,
            {
                match self {
                    $par_iter::Tight(iter) => bridge(iter, consumer),
                    $par_iter::Loose(iter) => bridge(iter, consumer),
                    $par_iter::Update(iter) => iter.drive_unindexed(consumer),
                    $par_iter::NonPacked(iter) => iter.drive_unindexed(consumer),
                }
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($par_iter: ident)*; $par_iter1: ident $($queue_par_iter: ident)+;
        $($par_tight: ident)*; $par_tight1: ident $($queue_par_tight: ident)+;
        $($par_loose: ident)*; $par_loose1: ident $($queue_par_loose: ident)+;
        $($par_non_packed: ident)*; $par_non_packed1: ident $($queue_par_non_packed: ident)+;
        $($par_update: ident)*; $par_update1: ident $($queue_par_update: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $par_iter1 $par_tight1 $par_loose1 $par_non_packed1 $par_update1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($par_iter)* $par_iter1; $($queue_par_iter)+;
            $($par_tight)* $par_tight1; $($queue_par_tight)+;
            $($par_loose)* $par_loose1; $($queue_par_loose)+;
            $($par_non_packed)* $par_non_packed1; $($queue_par_non_packed)+;
            $($par_update)* $par_update1; $($queue_par_update)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($par_iter: ident)*; $par_iter1: ident;
        $($par_tight: ident)*; $par_tight1: ident;
        $($par_loose: ident)*; $par_loose1: ident;
        $($par_non_packed: ident)*; $par_non_packed1: ident;
        $($par_update: ident)*; $par_update1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $par_iter1 $par_tight1 $par_loose1 $par_non_packed1 $par_update1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;ParIter2 ParIter3 ParIter4 ParIter5 ParIter6 ParIter7 ParIter8 ParIter9 ParIter10;
    ;ParTight2 ParTight3 ParTight4 ParTight5 ParTight6 ParTight7 ParTight8 ParTight9 ParTight10;
    ;ParLoose2 ParLoose3 ParLoose4 ParLoose5 ParLoose6 ParLoose7 ParLoose8 ParLoose9 ParLoose10;
    ;ParNonPacked2 ParNonPacked3 ParNonPacked4 ParNonPacked5 ParNonPacked6 ParNonPacked7 ParNonPacked8 ParNonPacked9 ParNonPacked10;
    ;ParUpdate2 ParUpdate3 ParUpdate4 ParUpdate5 ParUpdate6 ParUpdate7 ParUpdate8 ParUpdate9 ParUpdate10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
