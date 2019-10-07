use super::m_chunk::*;
use super::m_chunk_exact::*;
use super::m_tight_filter::*;
use super::m_tight_with_id::*;
use super::{AbstractMut, IntoAbstract};
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::Producer;

macro_rules! impl_iterators {
    (
        $number: literal
        $tight: ident
        $tight_filter: ident
        $tight_with_id: ident
        $chunk: ident
        $chunk_exact: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[doc = "Tight iterator over"]
        #[doc = $number]
        #[doc = "components.\n Tight iterators are fast but are limited to components tightly packed together."]
        pub struct $tight<$($type: IntoAbstract),+> {
            pub(super) data: ($($type::AbsView,)+),
            pub(super) current: usize,
            pub(super) end: usize,
        }

        impl<$($type: IntoAbstract),+> $tight<$($type),+> {
            pub fn filtered<P: FnMut(&<Self as Iterator>::Item) -> bool>(self, pred: P) -> $tight_filter<$($type),+, P> {
                $tight_filter {
                    iter: self,
                    pred
                }
            }
            pub fn with_id(self) -> $tight_with_id<$($type,)+> {
                $tight_with_id(self)
            }
        }

        impl<$($type: IntoAbstract),+> $tight<$($type),+> {
            /// Transform the iterator into a chunk iterator, returning `size` items at a time.
            ///
            /// Chunk will return a smaller slice at the end if `size` does not divide exactly the length.
            pub fn into_chunk(self, size: usize) -> $chunk<$($type),+> {
                $chunk {
                    data: self.data,
                    current: self.current,
                    end: self.end,
                    step: size,
                }
            }
            /// Transform the iterator into a chunk exact iterator, returning `size` items at a time.
            ///
            /// ChunkExact will always return a slice with the same length.
            ///
            /// To get the remaining items (if any) use the `remainder` method.
            pub fn into_chunk_exact(self, size: usize) -> $chunk_exact<$($type),+> {
                $chunk_exact {
                    data: self.data,
                    current: self.current,
                    end: self.end,
                    step: size,
                }
            }
        }

        impl<$($type: IntoAbstract),+> Iterator for $tight<$($type),+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn next(&mut self) -> Option<Self::Item> {
                let current = self.current;
                if current < self.end {
                    self.current += 1;
                    // SAFE the index is valid and the iterator can only be created where the lifetime is valid
                    Some(unsafe { ($(self.data.$index.get_data(current),)+) })
                } else {
                    None
                }
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                (self.len(), Some(self.len()))
            }
        }

        impl<$($type: IntoAbstract),+> DoubleEndedIterator for $tight<$($type),+> {
            fn next_back(&mut self) -> Option<Self::Item> {
                if self.end > self.current {
                    self.end -= 1;
                    // SAFE the index is valid and the iterator can only be created where the lifetime is valid
                    Some(unsafe { ($(self.data.$index.get_data(self.end),)+) })
                } else {
                    None
                }
            }
        }

        #[cfg(feature = "parallel")]
        impl<$($type: IntoAbstract),+> Producer for $tight<$($type),+>
        where
            $(<$type::AbsView as AbstractMut>::Out: Send),+
        {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            type IntoIter = Self;
            fn into_iter(self) -> Self::IntoIter {
                self
            }
            fn split_at(mut self, index: usize) -> (Self, Self) {
                let clone = $tight {
                    data: self.data.clone(),
                    current: self.current + index,
                    end: self.end,
                };
                self.end = clone.current;
                (self, clone)
            }
        }

        impl<$($type: IntoAbstract),+> ExactSizeIterator for $tight<$($type),+> {
            fn len(&self) -> usize {
                self.end - self.current
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($tight: ident)*; $tight1: ident $($queue_tight: ident)+;
        $($tight_with_id: ident)*; $tight_with_id1: ident $($queue_tight_with_id: ident)+;
        $($tight_filter: ident)*; $tight_filter1: ident $($queue_tight_filter: ident)+;
        $($chunk: ident)*; $chunk1: ident $($queue_chunk: ident)+;
        $($chunk_exact: ident)*; $chunk_exact1: ident $($queue_chunk_exact: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $tight1 $tight_filter1 $tight_with_id1 $chunk1 $chunk_exact1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($tight)* $tight1; $($queue_tight)+;
            $($tight_with_id)* $tight_with_id1; $($queue_tight_with_id)+;
            $($tight_filter)* $tight_filter1; $($queue_tight_filter)+;
            $($chunk)* $chunk1; $($queue_chunk)+;
            $($chunk_exact)* $chunk_exact1; $($queue_chunk_exact)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($tight: ident)*; $tight1: ident;
        $($tight_with_id: ident)*; $tight_with_id1: ident;
        $($tight_filter: ident)*; $tight_filter1: ident;
        $($chunk: ident)*; $chunk1: ident;
        $($chunk_exact: ident)*; $chunk_exact1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $tight1 $tight_filter1 $tight_with_id1 $chunk1 $chunk_exact1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Tight2 Tight3 Tight4 Tight5 Tight6 Tight7 Tight8 Tight9 Tight10;
    ;TightWithId2 TightWithId3 TightWithId4 TightWithId5 TightWithId6 TightWithId7 TightWithId8 TightWithId9 TightWithId10;
    ;TightFilter2 TightFilter3 TightFilter4 TightFilter5 TightFilter6 TightFilter7 TightFilter8 TightFilter9 TightFilter10;
    ;Chunk2 Chunk3 Chunk4 Chunk5 Chunk6 Chunk7 Chunk8 Chunk9 Chunk10;
    ;ChunkExact2 ChunkExact3 ChunkExact4 ChunkExact5 ChunkExact6 ChunkExact7 ChunkExact8 ChunkExact9 ChunkExact10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
