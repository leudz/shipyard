use super::{AbstractMut, IntoAbstract, IntoIter};
use rayon::iter::plumbing::{
    bridge_producer_consumer, bridge_unindexed, Folder, Producer, UnindexedConsumer,
    UnindexedProducer,
};
use rayon::iter::ParallelIterator;
use std::marker::PhantomData;

// Packed iterators go from start to end without index lookup
// They only work in specific circumstances but are the fastest
/// Iterator over a single component.
pub struct Packed1<T: IntoAbstract> {
    data: T::AbsView,
    current: usize,
    end: usize,
}

impl<T: IntoAbstract> Packed1<T> {
    /// Transform the iterator into a chunk iterator, returning multiple items.
    ///
    /// Chunk will return a smaller slice at the end if `size` does not divide exactly the length.
    pub fn into_chunk(self, size: usize) -> Chunk1<T> {
        Chunk1 {
            data: self.data,
            current: self.current,
            end: self.end,
            step: size,
        }
    }
    /// Transform the iterator into a chunk exact iterator, returning multiple items.
    ///
    /// ChunkExact will always return a slice with the same length.
    ///
    /// To get the remaining items (if any) use the `remainder` method.
    pub fn into_chunk_exact(self, size: usize) -> ChunkExact1<T> {
        ChunkExact1 {
            data: self.data,
            current: self.current,
            end: self.end,
            step: size,
        }
    }
}

impl<T: IntoAbstract> IntoIter for T {
    type IntoIter = Packed1<Self>;
    type IntoParIter = ParPacked1<Self>;
    fn iter(self) -> Self::IntoIter {
        Packed1 {
            end: self.indices().1.unwrap_or(0),
            data: self.into_abstract(),
            current: 0,
        }
    }
    fn par_iter(self) -> Self::IntoParIter {
        ParPacked1(self.iter())
    }
}

impl<T: IntoAbstract> IntoIter for (T,) {
    type IntoIter = Packed1<T>;
    type IntoParIter = ParPacked1<T>;
    fn iter(self) -> Self::IntoIter {
        T::iter(self.0)
    }
    fn par_iter(self) -> Self::IntoParIter {
        ParPacked1(self.iter())
    }
}

impl<T: IntoAbstract> Iterator for Packed1<T> {
    type Item = <T::AbsView as AbstractMut>::Out;
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        if current < self.end {
            self.current += 1;
            // SAFE the index is valid and the iterator can only be created where the lifetime is valid
            Some(unsafe { self.data.get_data(current) })
        } else {
            None
        }
    }
}

impl<T: IntoAbstract> DoubleEndedIterator for Packed1<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.end > self.current {
            self.end -= 1;
            // SAFE the index is valid and the iterator can only be created where the lifetime is valid
            Some(unsafe { self.data.get_data(self.end) })
        } else {
            None
        }
    }
}

impl<T: IntoAbstract> ExactSizeIterator for Packed1<T> {
    fn len(&self) -> usize {
        self.end - self.current
    }
}
/// Parallel iterator over a single component.
pub struct ParPacked1<T: IntoAbstract>(Packed1<T>);

impl<T: IntoAbstract> ParPacked1<T> {
    /// Trasnform this parallel iterator into its sequential version.
    pub fn into_seq(self) -> Packed1<T> {
        self.0
    }
}

impl<T: IntoAbstract> Producer for Packed1<T>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    type Item = <T::AbsView as AbstractMut>::Out;
    type IntoIter = Self;
    fn into_iter(self) -> Self::IntoIter {
        self
    }
    fn split_at(mut self, index: usize) -> (Self, Self) {
        let mut clone = Packed1 {
            data: self.data.clone(),
            current: self.current,
            end: self.end,
        };
        self.end -= index;
        clone.current += index;
        (self, clone)
    }
}

impl<T: IntoAbstract> ParallelIterator for ParPacked1<T>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    type Item = <T::AbsView as AbstractMut>::Out;
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        bridge_producer_consumer(self.0.len(), self.0, consumer)
    }
}

/// Chunk iterator over a single component.
///
/// Returns slice and not single elements.
///
/// The last chunk's length will be smaller than `size` if `size` does not divide the iterator's length perfectly.
pub struct Chunk1<T: IntoAbstract> {
    data: T::AbsView,
    current: usize,
    end: usize,
    step: usize,
}

impl<T: IntoAbstract> Iterator for Chunk1<T> {
    type Item = <T::AbsView as AbstractMut>::Slice;
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        if current + self.step < self.end {
            self.current += self.step;
            Some(unsafe { self.data.get_data_slice(current..(current + self.step)) })
        } else if current < self.end {
            self.current = self.end;
            Some(unsafe { self.data.get_data_slice(current..self.end) })
        } else {
            None
        }
    }
}

/// Chunk iterator over a single component.
///
/// Returns a slice and not single elements.
/// One of the benefit is to allow the use of SIMD instructions.
pub struct ChunkExact1<T: IntoAbstract> {
    data: T::AbsView,
    current: usize,
    end: usize,
    step: usize,
}

impl<T: IntoAbstract> ChunkExact1<T> {
    /// Returns the items at the end of the slice.
    ///
    /// Will always return a slice smaller than `size`.
    pub fn remainder(&mut self) -> <T::AbsView as AbstractMut>::Slice {
        let remainder = std::cmp::min(self.end - self.current, self.end % self.step);
        self.end -= remainder;
        unsafe { self.data.get_data_slice((self.end - remainder)..self.end) }
    }
}

impl<T: IntoAbstract> Iterator for ChunkExact1<T> {
    type Item = <T::AbsView as AbstractMut>::Slice;
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        if current + self.step < self.end {
            self.current += self.step;
            Some(unsafe { self.data.get_data_slice(current..(current + self.step)) })
        } else {
            None
        }
    }
}

macro_rules! impl_iterators {
    ($iter: ident $par_iter: ident $packed: ident $non_packed: ident $chunk: ident $chunk_exact: ident $par_packed: ident $par_non_packed: ident $(($type: ident, $index: tt))+) => {
        /// Packed iterator over multiple components.
        ///
        /// Packed owned iterators are fast but are limited to components packed together.
        pub struct $packed<$($type: IntoAbstract),+> {
            data: ($($type::AbsView,)+),
            current: usize,
            end: usize,
        }

        impl<$($type: IntoAbstract),+> $packed<$($type),+> {
            /// Transform the iterator into a chunk iterator, returning multiple items.
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
            /// Transform the iterator into a chunk exact iterator, returning multiple items.
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

        /// Parallel iterator over multiple components.
        ///
        /// Packed owned iterators are fast but are limited to components packed together.
        pub struct $par_packed<$($type: IntoAbstract),+>($packed<$($type),+>);

        /// Chunk iterator over multiple components.
        ///
        /// Returns a tuple of slices and not single element.
        ///
        /// The last chunk's length will be smaller than `size` if `size` does not divide the iterator's length perfectly.
        pub struct $chunk<$($type: IntoAbstract),+> {
            data: ($($type::AbsView,)+),
            current: usize,
            end: usize,
            step: usize,
        }

        /// Chunk iterator over multiple components.
        ///
        /// Returns a tuple of slices and not single element.
        /// One of the benefit is to allow the use of SIMD instructions.
        pub struct $chunk_exact<$($type: IntoAbstract),+> {
            data: ($($type::AbsView,)+),
            current: usize,
            end: usize,
            step: usize,
        }

        impl<$($type: IntoAbstract),+> $chunk_exact<$($type),+> {
            /// Returns the items at the end of the slice.
            ///
            /// Will always return a slice smaller than `size`.
            pub fn remainder(&mut self) -> ($(<$type::AbsView as AbstractMut>::Slice,)+) {
                let end = self.end;
                let remainder = std::cmp::min(self.end - self.current, self.end % self.step);
                self.end -= remainder;
                ($(
                    unsafe { self.data.$index.get_data_slice((end - remainder)..end) },
                )+)
            }
        }

        /// Non packed iterator over multiple components.
        pub struct $non_packed<$($type: IntoAbstract),+> {
            data: ($($type::AbsView,)+),
            indices: *const usize,
            current: usize,
            end: usize,
            array: usize,
        }

        unsafe impl<$($type: IntoAbstract),+> Send for $non_packed<$($type),+> {}

        impl<$($type: IntoAbstract),+> Clone for $non_packed<$($type),+> {
            fn clone(&self) -> Self {
                $non_packed {
                    data: self.data.clone(),
                    indices: self.indices,
                    current: self.current,
                    end: self.end,
                    array: self.array,
                }
            }
        }

        /// Parallel non packed iterator over multiple components.
        pub struct $par_non_packed<$($type: IntoAbstract),+>($non_packed<$($type),+>);

        /// Iterator over multiple components.
        ///
        /// The enum allows to abstract away what kind of iterator you really get.
        /// That doesn't mean the performance will suffer.
        pub enum $iter<$($type: IntoAbstract),+> {
            Packed($packed<$($type),+>),
            NonPacked($non_packed<$($type),+>),
        }

        impl<$($type: IntoAbstract),+> $iter<$($type),+> {
            /// Tries to transform the iterator into a chunk iterator, returning multiple items.
            /// If the components are not packed together the iterator is returned.
            ///
            /// Chunk will return a smaller slice at the end if `size` does not divide exactly the length.
            pub fn into_chunk(self, size: usize) -> Result<$chunk<$($type),+>, Self> {
                match self {
                    $iter::Packed(iter) => Ok(iter.into_chunk(size)),
                    $iter::NonPacked(_) => Err(self),
                }
            }
            /// Tries to transform the iterator into a chunk exact iterator, returning multiple items.
            /// If the components are not packed together the iterator is returned.
            ///
            /// ChunkExact will always return a slice with the same length.
            ///
            /// To get the remaining items (if any) use the `remainder` method.
            pub fn into_chunk_exact(self, size: usize) -> Result<$chunk_exact<$($type),+>, Self> {
                match self {
                    $iter::Packed(iter) => Ok(iter.into_chunk_exact(size)),
                    $iter::NonPacked(_) => Err(self),
                }
            }
        }

        impl<$($type: IntoAbstract),+> Iterator for $iter<$($type),+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn next(&mut self) -> Option<Self::Item> {
                match self {
                    $iter::Packed(iter) => iter.next(),
                    $iter::NonPacked(iter) => iter.next(),
                }
            }
        }

        /// Parallel iterator over multiple components.
        ///
        /// The enum allows to abstract away what kind of iterator you really get.
        /// That doesn't mean the performance will suffer.
        pub enum $par_iter<$($type: IntoAbstract),+> {
            Packed($par_packed<$($type),+>),
            NonPacked($par_non_packed<$($type),+>),
        }

        impl<$($type: IntoAbstract),+> ParallelIterator for $par_iter<$($type),+>
        where $(<$type::AbsView as AbstractMut>::Out: Send),+ {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn drive_unindexed<Cons>(self, consumer: Cons) -> Cons::Result
            where
                Cons: UnindexedConsumer<Self::Item>,
            {
                match self {
                    $par_iter::Packed(iter) => bridge_producer_consumer(iter.0.len(), iter.0, consumer),
                    $par_iter::NonPacked(iter) => bridge_unindexed(iter.0, consumer),
                }
            }
        }

        impl<$($type: IntoAbstract),+> IntoIter for ($($type,)+) {
            type IntoIter = $iter<$($type,)+>;
            type IntoParIter = $par_iter<$($type,)+>;
            fn iter(self) -> Self::IntoIter {
                // check if all types are packed together
                let packed_types = self.0.abs_pack_types_owned();
                let mut i = 0;
                $(let _: PhantomData<$type> = {i += 1; PhantomData};)+
                if $(std::ptr::eq(packed_types, self.$index.abs_pack_types_owned()))&&+ && i == packed_types.len() {
                    $iter::Packed($packed {
                        end: self.0.abs_pack_len(),
                        data: ($(self.$index.into_abstract(),)+),
                        current: 0,
                    })
                } else {
                    let mut smallest_index = std::usize::MAX;
                    let mut i = 0;
                    let mut tuple: (*const usize, usize) = (&0, std::usize::MAX);
                        $({
                            let new_tuple = self.$index.indices();
                            if let Some(new_len) = new_tuple.1 {
                                if new_len < tuple.1 {
                                    smallest_index = i;
                                    tuple = (new_tuple.0, new_len);
                                }
                            }
                            i += 1;
                        })+
                    let _ = i;

                    // if the user is trying to iterate over Not containers only
                    if tuple.1 == std::usize::MAX {
                        tuple.1 = 0;
                    }

                    $iter::NonPacked($non_packed {
                        data: ($(self.$index.into_abstract(),)+),
                        indices: tuple.0,
                        current: 0,
                        end: tuple.1,
                        array: smallest_index,
                    })
                }
            }
            fn par_iter(self) -> Self::IntoParIter {
                match self.iter() {
                    $iter::Packed(iter) => $par_iter::Packed($par_packed(iter)),
                    $iter::NonPacked(iter) => $par_iter::NonPacked($par_non_packed(iter)),
                }
            }
        }

        impl<$($type: IntoAbstract),+> Iterator for $packed<$($type),+> {
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
        }

        impl<$($type: IntoAbstract),+> DoubleEndedIterator for $packed<$($type),+> {
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

        impl<$($type: IntoAbstract),+> ExactSizeIterator for $packed<$($type),+> {
            fn len(&self) -> usize {
                self.end - self.current
            }
        }

        impl<$($type: IntoAbstract),+> Producer for $packed<$($type),+>
        where
            $(<$type::AbsView as AbstractMut>::Out: Send),+
        {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            type IntoIter = Self;
            fn into_iter(self) -> Self::IntoIter {
                self
            }
            fn split_at(mut self, index: usize) -> (Self, Self) {
                let mut clone = $packed {
                    data: self.data.clone(),
                    current: self.current,
                    end: self.end,
                };
                self.end -= index;
                clone.current += index;
                (self, clone)
            }
        }

        impl<$($type: IntoAbstract),+> ParallelIterator for $par_packed<$($type),+>
        where
            $(<$type::AbsView as AbstractMut>::Out: Send),+
        {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn drive_unindexed<Cons>(self, consumer: Cons) -> Cons::Result
            where
                Cons: UnindexedConsumer<Self::Item>,
            {
                bridge_producer_consumer(self.0.len(), self.0, consumer)
            }
        }

        impl<$($type: IntoAbstract),+> Iterator for $non_packed<$($type,)+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn next(&mut self) -> Option<Self::Item> {
                while self.current < self.end {
                    // SAFE at this point there are no mutable reference to sparse or dense
                    // and self.indices can't access out of bounds
                    let index: usize = unsafe { std::ptr::read(self.indices.add(self.current)) };
                    self.current += 1;
                    return Some(($({
                        if $index == self.array {
                            // SAFE the index is valid and the iterator can only be created where the lifetime is valid
                            unsafe { self.data.$index.get_data(self.current - 1) }
                        } else {
                            // SAFE the index is valid and the iterator can only be created where the lifetime is valid
                            if let Some(item) = unsafe { self.data.$index.abs_get(index) } {
                                item
                            } else {
                                continue
                            }
                        }
                    },)+))
                }
                None
            }
        }

        impl<$($type: IntoAbstract),+> UnindexedProducer for $non_packed<$($type,)+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn split(mut self) -> (Self, Option<Self>) {
                let len = self.end - self.current;
                if self.end - self.current >= 2 {
                    let mut clone = self.clone();
                    self.end -= len / 2;
                    clone.current += len / 2;
                    (self, Some(clone))
                } else {
                    (self, None)
                }
            }
            fn fold_with<F>(mut self, mut folder: F) -> F
            where F: Folder<Self::Item> {
                while !folder.full() {
                    if let Some(item) = self.next() {
                        folder = folder.consume(item);
                    } else {
                        break;
                    }
                }
                folder
            }
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

        impl<$($type: IntoAbstract),+> Iterator for $chunk_exact<$($type),+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Slice,)+);
            fn next(&mut self) -> Option<Self::Item> {
                let current = self.current;
                if current + self.step <= self.end {
                    self.current += self.step;
                    Some(($(unsafe { self.data.$index.get_data_slice(current..(current + self.step)) },)+))
                } else {
                    None
                }
            }
        }
    }
}

macro_rules! iterators {
    (
        $($iter: ident)*; $iter1: ident $($queue_iter: ident)+;
        $($par_iter: ident)*; $par_iter1: ident $($queue_par_iter: ident)+;
        $($packed: ident)*; $packed1: ident $($queue_packed: ident)+;
        $($non_packed: ident)*; $non_packed1: ident $($queue_non_packed: ident)+;
        $($chunk: ident)*; $chunk1: ident $($queue_chunk: ident)+;
        $($chunk_exact: ident)*; $chunk_exact1: ident $($queue_chunk_exact: ident)+;
        $($par_packed: ident)*; $par_packed1: ident $($queue_par_packed: ident)+;
        $($par_non_packed: ident)*; $par_non_packed1: ident $($queue_par_non_packed: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$iter1 $par_iter1 $packed1 $non_packed1 $chunk1 $chunk_exact1 $par_packed1 $par_non_packed1 $(($type, $index))*];
        iterators![
            $($iter)* $iter1; $($queue_iter)+;
            $($par_iter)* $par_iter1; $($queue_par_iter)+;
            $($packed)* $packed1; $($queue_packed)+;
            $($non_packed)* $non_packed1; $($queue_non_packed)+;
            $($chunk)* $chunk1; $($queue_chunk)+;
            $($chunk_exact)* $chunk_exact1; $($queue_chunk_exact)+;
            $($par_packed)* $par_packed1; $($queue_par_packed)+;
            $($par_non_packed)* $par_non_packed1; $($queue_par_non_packed)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($iter: ident)*; $iter1: ident;
        $($par_iter: ident)*; $par_iter1: ident;
        $($packed: ident)*; $packed1: ident;
        $($non_packed: ident)*; $non_packed1: ident;
        $($chunk: ident)*; $chunk1: ident;
        $($chunk_exact: ident)*; $chunk_exact1: ident;
        $($par_packed: ident)*; $par_packed1: ident;
        $($par_non_packed: ident)*; $par_non_packed1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$iter1 $par_iter1 $packed1 $non_packed1 $chunk1 $chunk_exact1 $par_packed1 $par_non_packed1 $(($type, $index))*];
    }
}

iterators![
    ;Iter2 Iter3 Iter4 Iter5;
    ;ParIter2 ParIter3 ParIter4 ParIter5;
    ;Packed2 Packed3 Packed4 Packed5;
    ;NonPacked2 NonPacked3 NonPacked4 NonPacked5;
    ;Chunk2 Chunk3 Chunk4 Chunk5;
    ;ChunkExact2 ChunkExact3 ChunkExact4 ChunkExact5;
    ;ParPacked2 ParPacked3 ParPacked4 ParPacked5;
    ;ParNonPacked2 ParNonPacked3 ParNonPacked4 ParNonPacked5;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/
];
