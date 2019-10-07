use super::m_chunk::*;
use super::m_chunk_exact::*;
use super::m_filter::*;
use super::m_loose::*;
use super::m_non_packed::*;
use super::m_tight::*;
use super::m_update::*;
use super::m_with_id::*;
use super::{AbstractMut, IntoAbstract};

macro_rules! impl_iterators {
    (
        $number: literal
        $iter: ident
        $tight: ident
        $loose: ident
        $non_packed: ident
        $chunk: ident
        $chunk_exact: ident
        $update: ident
        $filter: ident
        $with_id: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[doc = "Iterator over"]
        #[doc = $number]
        #[doc = "components.\n This enum allows to abstract away what kind of iterator you really get. That doesn't mean the performance will suffer, the compiler will (almost)
        always optimize it away."]
        pub enum $iter<$($type: IntoAbstract),+> {
            Tight($tight<$($type),+>),
            Loose($loose<$($type),+>),
            Update($update<$($type),+>),
            NonPacked($non_packed<$($type),+>),
        }

        impl<$($type: IntoAbstract),+> $iter<$($type),+> {
            /// Tries to transform the iterator into a chunk iterator, returning `size` items at a time.
            /// If the components are not tightly packed together the iterator is returned.
            ///
            /// Chunk will return a smaller slice at the end if `size` does not divide exactly the length.
            pub fn into_chunk(self, size: usize) -> Result<$chunk<$($type),+>, Self> {
                match self {
                    $iter::Tight(iter) => Ok(iter.into_chunk(size)),
                    $iter::Loose(_) => Err(self),
                    $iter::Update(_) => Err(self),
                    $iter::NonPacked(_) => Err(self),
                }
            }
            /// Tries to transform the iterator into a chunk exact iterator, returning `size` items at a time.
            /// If the components are not tightly packed together the iterator is returned.
            ///
            /// ChunkExact will always return a slice with the same length.
            ///
            /// To get the remaining items (if any) use the `remainder` method.
            pub fn into_chunk_exact(self, size: usize) -> Result<$chunk_exact<$($type),+>, Self> {
                match self {
                    $iter::Tight(iter) => Ok(iter.into_chunk_exact(size)),
                    $iter::Loose(_) => Err(self),
                    $iter::Update(_) => Err(self),
                    $iter::NonPacked(_) => Err(self),
                }
            }
            pub fn filtered<P: FnMut(&<Self as Iterator>::Item) -> bool>(self, pred: P) -> $filter<$($type),+, P> {
                match self {
                    $iter::Tight(iter) => $filter::Tight(iter.filtered(pred)),
                    $iter::Loose(iter) => $filter::Loose(iter.filtered(pred)),
                    $iter::Update(iter) => $filter::Update(iter.filtered(pred)),
                    $iter::NonPacked(iter) => $filter::NonPacked(iter.filtered(pred)),
                }
            }
            pub fn with_id(self) -> $with_id<$($type),+> {
                match self {
                    $iter::Tight(iter) => $with_id::Tight(iter.with_id()),
                    $iter::Loose(iter) => $with_id::Loose(iter.with_id()),
                    $iter::Update(iter) => $with_id::Update(iter.with_id()),
                    $iter::NonPacked(iter) => $with_id::NonPacked(iter.with_id()),
                }
            }
        }

        impl<$($type: IntoAbstract),+> Iterator for $iter<$($type),+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn next(&mut self) -> Option<Self::Item> {
                match self {
                    $iter::Tight(iter) => iter.next(),
                    $iter::Loose(iter) => iter.next(),
                    $iter::Update(iter) => iter.next(),
                    $iter::NonPacked(iter) => iter.next(),
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
        $($chunk: ident)*; $chunk1: ident $($queue_chunk: ident)+;
        $($chunk_exact: ident)*; $chunk_exact1: ident $($queue_chunk_exact: ident)+;
        $($update: ident)*; $update1: ident $($queue_update: ident)+;
        $($filter: ident)*; $filter1: ident $($queue_filter: ident)+;
        $($with_id: ident)*; $with_id1: ident $($queue_with_id: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $iter1 $tight1 $loose1 $non_packed1 $chunk1 $chunk_exact1 $update1 $filter1 $with_id1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($iter)* $iter1; $($queue_iter)+;
            $($tight)* $tight1; $($queue_tight)+;
            $($loose)* $loose1; $($queue_loose)+;
            $($non_packed)* $non_packed1; $($queue_non_packed)+;
            $($chunk)* $chunk1; $($queue_chunk)+;
            $($chunk_exact)* $chunk_exact1; $($queue_chunk_exact)+;
            $($update)* $update1; $($queue_update)+;
            $($filter)* $filter1; $($queue_filter)+;
            $($with_id)* $with_id1; $($queue_with_id)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($iter: ident)*; $iter1: ident;
        $($tight: ident)*; $tight1: ident;
        $($loose: ident)*; $loose1: ident;
        $($non_packed: ident)*; $non_packed1: ident;
        $($chunk: ident)*; $chunk1: ident;
        $($chunk_exact: ident)*; $chunk_exact1: ident;
        $($update: ident)*; $update1: ident;
        $($filter: ident)*; $filter1: ident;
        $($with_id: ident)*; $with_id1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $iter1 $tight1 $loose1 $non_packed1 $chunk1 $chunk_exact1 $update1 $filter1 $with_id1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Iter2 Iter3 Iter4 Iter5 Iter6 Iter7 Iter8 Iter9 Iter10;
    ;Tight2 Tight3 Tight4 Tight5 Tight6 Tight7 Tight8 Tight9 Tight10;
    ;Loose2 Loose3 Loose4 Loose5 Loose6 Loose7 Loose8 Loose9 Loose10;
    ;NonPacked2 NonPacked3 NonPacked4 NonPacked5 NonPacked6 NonPacked7 NonPacked8 NonPacked9 NonPacked10;
    ;Chunk2 Chunk3 Chunk4 Chunk5 Chunk6 Chunk7 Chunk8 Chunk9 Chunk10;
    ;ChunkExact2 ChunkExact3 ChunkExact4 ChunkExact5 ChunkExact6 ChunkExact7 ChunkExact8 ChunkExact9 ChunkExact10;
    ;Update2 Update3 Update4 Update5 Update6 Update7 Update8 Update9 Update10;
    ;Filter2 Filter3 Filter4 Filter5 Filter6 Filter7 Filter8 Filter9 Filter10;
    ;WithId2 WithId3 WithId4 WithId5 WithId6 WithId7 WithId8 WithId9 WithId10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
