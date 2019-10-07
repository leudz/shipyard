use super::{AbstractMut, IntoAbstract};

macro_rules! impl_iterators {
    (
        $number: literal
        $chunk: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[doc = "Chunk iterator over"]
        #[doc = $number]
        #[doc = "components.\n Returns a tuple of `size` long slices and not single elements.\n The last chunk's length will be smaller than `size` if `size` does not divide the iterator's length perfectly."]
        pub struct $chunk<$($type: IntoAbstract),+> {
            pub(super) data: ($($type::AbsView,)+),
            pub(super) current: usize,
            pub(super) end: usize,
            pub(super) step: usize,
        }

        impl<$($type: IntoAbstract),+> Iterator for $chunk<$($type),+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Slice,)+);
            fn next(&mut self) -> Option<Self::Item> {
                let current = self.current;
                if current + self.step <= self.end {
                    self.current += self.step;
                    Some(($(unsafe { self.data.$index.get_data_slice(current..(current + self.step)) },)+))
                } else if current < self.end {
                    self.current = self.end;
                    Some(($(unsafe { self.data.$index.get_data_slice(current..self.end) },)+))
                } else {
                    None
                }
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($chunk: ident)*; $chunk1: ident $($queue_chunk: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $chunk1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($chunk)* $chunk1; $($queue_chunk)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($chunk: ident)*; $chunk1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $chunk1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Chunk2 Chunk3 Chunk4 Chunk5 Chunk6 Chunk7 Chunk8 Chunk9 Chunk10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
