use super::*;
use rayon::iter::plumbing::UnindexedConsumer;
use rayon::iter::{IndexedParallelIterator, ParallelIterator};

macro_rules! impl_iterators {
    (
        $number: literal
        $iter: ident
        $tight: ident
        $loose: ident
        $non_packed: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[doc = "Parallel iterator over"]
        #[doc = $number]
        #[doc = "components.  
This enum allows to abstract away what kind of iterator you really get. That doesn't mean the performance will suffer, the compiler will (almost) always optimize it away."]
        pub enum $iter<$($type: IntoAbstract),+> {
            Tight($tight<$($type),+>),
            Loose($loose<$($type),+>),
            NonPacked($non_packed<$($type),+>),
        }

        impl<$($type: IntoAbstract),+> ParallelIterator for $iter<$($type),+>
        where $($type::AbsView: Clone + Send,)+ $(<$type::AbsView as AbstractMut>::Out: Send),+ {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn drive_unindexed<Con>(self, consumer: Con) -> Con::Result where Con: UnindexedConsumer<Self::Item> {
                match self {
                    Self::Tight(tight) => tight.drive(consumer),
                    Self::Loose(loose) => loose.drive(consumer),
                    Self::NonPacked(non_packed) => non_packed.drive_unindexed(consumer),
                }
            }
            fn opt_len(&self) -> Option<usize> {
                match self {
                    Self::Tight(tight) => tight.opt_len(),
                    Self::Loose(loose) => loose.opt_len(),
                    Self::NonPacked(non_packed) => non_packed.opt_len(),
                }
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($iter: ident)*; $iter1: ident $($queue_iter: ident)+;
        $($tight: ident)*; $tight1: ident $($queue_tight: ident)+;
        $($loose: ident)*; $loose1: ident $($queue_loose: ident)+;
        $($non_packed: ident)*; $non_packed1: ident $($queue_non_packed: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $iter1 $tight1 $loose1 $non_packed1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($iter)* $iter1; $($queue_iter)+;
            $($tight)* $tight1; $($queue_tight)+;
            $($loose)* $loose1; $($queue_loose)+;
            $($non_packed)* $non_packed1; $($queue_non_packed)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($iter: ident)*; $iter1: ident;
        $($tight: ident)*; $tight1: ident;
        $($loose: ident)*; $loose1: ident;
        $($non_packed: ident)*; $non_packed1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $iter1 $tight1 $loose1 $non_packed1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;ParIter2 ParIter3 ParIter4 ParIter5 ParIter6 ParIter7 ParIter8 ParIter9 ParIter10;
    ;ParTight2 ParTight3 ParTight4 ParTight5 ParTight6 ParTight7 ParTight8 ParTight9 ParTight10;
    ;ParLoose2 ParLoose3 ParLoose4 ParLoose5 ParLoose6 ParLoose7 ParLoose8 ParLoose9 ParLoose10;
    ;ParNonPacked2 ParNonPacked3 ParNonPacked4 ParNonPacked5 ParNonPacked6 ParNonPacked7 ParNonPacked8 ParNonPacked9 ParNonPacked10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
