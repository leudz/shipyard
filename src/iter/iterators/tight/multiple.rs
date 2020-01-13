use super::chunk::multiple::*;
use super::chunk_exact::multiple::*;
use super::{AbstractMut, CurrentId, IntoAbstract, Shiperator};
use crate::EntityId;

macro_rules! impl_iterators {
    (
        $number: literal
        $tight: ident
        $chunk: ident
        $chunk_exact: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[doc = "Tight iterator over"]
        #[doc = $number]
        #[doc = "components.\n Tight iterators are fast but are limited to components tightly packed together."]
        pub struct $tight<$($type: IntoAbstract),+> {
            pub(crate) data: ($($type::AbsView,)+),
            pub(crate) current: usize,
            pub(crate) end: usize,
        }

        impl<$($type: IntoAbstract),+> $tight<$($type),+> {
            pub fn into_chunk(self, step: usize) -> $chunk<$($type),+> {
                $chunk {
                    data: self.data,
                    current: self.current,
                    end: self.end,
                    step,
                }
            }
            pub fn into_chunk_exact(self, step: usize) -> $chunk_exact<$($type),+> {
                $chunk_exact {
                    data: self.data,
                    current: self.current,
                    end: self.end,
                    step,
                }
            }
        }

        impl<$($type: IntoAbstract),+> Shiperator for $tight<$($type),+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);

            fn first_pass(&mut self) -> Option<Self::Item> {
                let current = self.current;
                if current < self.end {
                    self.current += 1;
                    Some(unsafe {($(self.data.$index.get_data(current),)+)})
                } else {
                    None
                }
            }
            fn post_process(&mut self, item: Self::Item) -> Self::Item {
                item
            }
        }

        impl<$($type: IntoAbstract),+> CurrentId for $tight<$($type),+> {
            type Id = EntityId;

            unsafe fn current_id(&self) -> Self::Id {
                self.data.0.id_at(self.current - 1)
            }
        }

    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($tight: ident)*; $tight1: ident $($queue_tight: ident)+;
        $($chunk: ident)*; $chunk1: ident $($queue_chunk: ident)+;
        $($chunk_exact: ident)*; $chunk_exact1: ident $($queue_chunk_exact: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $tight1 $chunk1 $chunk_exact1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($tight)* $tight1; $($queue_tight)+;
            $($chunk)* $chunk1; $($queue_chunk)+;
            $($chunk_exact)* $chunk_exact1; $($queue_chunk_exact)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($tight: ident)*; $tight1: ident;
        $($chunk: ident)*; $chunk1: ident;
        $($chunk_exact: ident)*; $chunk_exact1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $tight1 $chunk1 $chunk_exact1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Tight2 Tight3 Tight4 Tight5 Tight6 Tight7 Tight8 Tight9 Tight10;
    ;Chunk2 Chunk3 Chunk4 Chunk5 Chunk6 Chunk7 Chunk8 Chunk9 Chunk10;
    ;ChunkExact2 ChunkExact3 ChunkExact4 ChunkExact5 ChunkExact6 ChunkExact7 ChunkExact8 ChunkExact9 ChunkExact10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
