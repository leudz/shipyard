use super::*;
use crate::EntityId;

macro_rules! impl_iterators {
    (
        $number: literal
        $iter: ident
        $tight: ident
        $chunk: ident
        $chunk_exact: ident
        $loose: ident
        $non_packed: ident
        $update: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[doc = "Iterator over"]
        #[doc = $number]
        #[doc = "components.  
This enum allows to abstract away what kind of iterator you really get. That doesn't mean the performance will suffer, the compiler will (almost) always optimize it away."]
        pub enum $iter<$($type: IntoAbstract),+> {
            Tight($tight<$($type),+>),
            Loose($loose<$($type),+>),
            Update($update<$($type),+>),
            NonPacked($non_packed<$($type),+>),
        }

        impl<$($type: IntoAbstract),+> $iter<$($type),+> {
            pub fn into_chunk(self, step: usize) -> Result<$chunk<$($type),+>, Self> {
                match self {
                    Self::Tight(tight) => Ok(tight.into_chunk(step)),
                    _ => Err(self)
                }
            }
            pub fn into_chunk_exact(self, step: usize) -> Result<$chunk_exact<$($type),+>, Self> {
                match self {
                    Self::Tight(tight) => Ok(tight.into_chunk_exact(step)),
                    _ => Err(self)
                }
            }
        }

        impl<$($type: IntoAbstract),+> Shiperator for $iter<$($type),+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);

            fn first_pass(&mut self) -> Option<Self::Item> {
                match self {
                    Self::Tight(tight) => tight.first_pass(),
                    Self::Loose(loose) => loose.first_pass(),
                    Self::Update(update) => update.first_pass(),
                    Self::NonPacked(non_packed) => non_packed.first_pass(),
                }
            }
            fn post_process(&mut self) {
                match self {
                    Self::Tight(tight) => tight.post_process(),
                    Self::Loose(loose) => loose.post_process(),
                    Self::Update(update) => update.post_process(),
                    Self::NonPacked(non_packed) => non_packed.post_process(),
                }
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                match self {
                    Self::Tight(tight) => tight.size_hint(),
                    Self::Loose(loose) => loose.size_hint(),
                    Self::Update(update) => update.size_hint(),
                    Self::NonPacked(non_packed) => non_packed.size_hint(),
                }
            }
        }
        impl<$($type: IntoAbstract),+> CurrentId for $iter<$($type),+> {
            type Id = EntityId;

            unsafe fn current_id(&self) -> Self::Id {
                match self {
                    Self::Tight(tight) => tight.current_id(),
                    Self::Loose(loose) => loose.current_id(),
                    Self::Update(update) => update.current_id(),
                    Self::NonPacked(non_packed) => non_packed.current_id(),
                }
            }
        }

        impl<$($type: IntoAbstract),+> core::iter::IntoIterator for $iter<$($type),+> {
            type IntoIter = IntoIterator<Self>;
            type Item = <Self as Shiperator>::Item;
            fn into_iter(self) -> Self::IntoIter {
                IntoIterator(self)
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($iter: ident)*; $iter1: ident $($queue_iter: ident)+;
        $($tight: ident)*; $tight1: ident $($queue_tight: ident)+;
        $($chunk: ident)*; $chunk1: ident $($queue_chunk: ident)+;
        $($chunk_exact: ident)*; $chunk_exact1: ident $($queue_chunk_exact: ident)+;
        $($loose: ident)*; $loose1: ident $($queue_loose: ident)+;
        $($non_packed: ident)*; $non_packed1: ident $($queue_non_packed: ident)+;
        $($update: ident)*; $update1: ident $($queue_update: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $iter1 $tight1 $chunk1 $chunk_exact1 $loose1 $non_packed1 $update1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($iter)* $iter1; $($queue_iter)+;
            $($tight)* $tight1; $($queue_tight)+;
            $($chunk)* $chunk1; $($queue_chunk)+;
            $($chunk_exact)* $chunk_exact1; $($queue_chunk_exact)+;
            $($loose)* $loose1; $($queue_loose)+;
            $($non_packed)* $non_packed1; $($queue_non_packed)+;
            $($update)* $update1; $($queue_update)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($iter: ident)*; $iter1: ident;
        $($tight: ident)*; $tight1: ident;
        $($chunk: ident)*; $chunk1: ident;
        $($chunk_exact: ident)*; $chunk_exact1: ident;
        $($loose: ident)*; $loose1: ident;
        $($non_packed: ident)*; $non_packed1: ident;
        $($update: ident)*; $update1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $iter1 $tight1 $chunk1 $chunk_exact1 $loose1 $non_packed1 $update1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Iter2 Iter3 Iter4 Iter5 Iter6 Iter7 Iter8 Iter9 Iter10;
    ;Tight2 Tight3 Tight4 Tight5 Tight6 Tight7 Tight8 Tight9 Tight10;
    ;Chunk2 Chunk3 Chunk4 Chunk5 Chunk6 Chunk7 Chunk8 Chunk9 Chunk10;
    ;ChunkExact2 ChunkExact3 ChunkExact4 ChunkExact5 ChunkExact6 ChunkExact7 ChunkExact8 ChunkExact9 ChunkExact10;
    ;Loose2 Loose3 Loose4 Loose5 Loose6 Loose7 Loose8 Loose9 Loose10;
    ;NonPacked2 NonPacked3 NonPacked4 NonPacked5 NonPacked6 NonPacked7 NonPacked8 NonPacked9 NonPacked10;
    ;Update2 Update3 Update4 Update5 Update6 Update7 Update8 Update9 Update10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
