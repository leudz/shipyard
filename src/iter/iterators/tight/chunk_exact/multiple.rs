use super::{AbstractMut, IntoAbstract, IntoIterator, Shiperator};

macro_rules! impl_iterators {
    (
        $number: literal
        $chunk_exact: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[doc = "Chunk iterator over"]
        #[doc = $number]
        #[doc = "components.  
Returns a tuple of `size` long slices and not single elements.  
ChunkExact will always return a slice with the same length.  
To get the remaining items (if any) use the `remainder` method."]
        pub struct $chunk_exact<$($type: IntoAbstract),+> {
            pub(crate) data: ($($type::AbsView,)+),
            pub(crate) current: usize,
            pub(crate) end: usize,
            pub(crate) step: usize,
        }

        impl<$($type: IntoAbstract),+> Shiperator for $chunk_exact<$($type),+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Slice,)+);
            fn first_pass(&mut self) -> Option<Self::Item> {
                let current = self.current;
                if current + self.step <= self.end {
                    self.current += self.step;
                    // SAFE we checked for OOB and the lifetime is ok
                    Some(unsafe {($(self.data.$index.get_data_slice(current..(current + self.step)),)+)})
                } else {
                    None
                }
            }
            fn post_process(&mut self) {

            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                let len = (self.end - self.current) / self.step;
                (len, Some(len))
            }
        }

        impl<$($type: IntoAbstract),+> $chunk_exact<$($type),+> {
            pub fn remainder(&mut self) -> ($(<$type::AbsView as AbstractMut>::Slice,)+) {
                let end = self.end;
                let remainder = core::cmp::min(self.end - self.current, self.end % self.step);
                self.end -= remainder;
                ($(
                    // SAFE we checked for OOB and the lifetime is ok
                    unsafe { self.data.$index.get_data_slice(self.end..end) },
                )+)
            }
        }

        impl<$($type: IntoAbstract),+> core::iter::IntoIterator for $chunk_exact<$($type),+> {
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
        $($chunk_exact: ident)*; $chunk_exact1: ident $($queue_chunk_exact: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $chunk_exact1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($chunk_exact)* $chunk_exact1; $($queue_chunk_exact)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($chunk_exact: ident)*; $chunk_exact1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $chunk_exact1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;ChunkExact2 ChunkExact3 ChunkExact4 ChunkExact5 ChunkExact6 ChunkExact7 ChunkExact8 ChunkExact9 ChunkExact10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
