use super::{AbstractMut, IntoAbstract, IntoIter};
use rayon::iter::plumbing::{
    bridge_producer_consumer, bridge_unindexed, Folder, Producer, UnindexedConsumer,
    UnindexedProducer,
};
use rayon::iter::ParallelIterator;

/// Iterator over components.
///
/// This enum allows to abstract away what kind of iterator you really get. That doesn't mean the performance will suffer.
pub enum Iter<T: IntoAbstract> {
    Packed(Packed<T>),
    NonPacked(NonPacked<T>),
}

impl<T: IntoAbstract> Iterator for Iter<T>
where
    T::AbsView: AbstractMut,
{
    type Item = <T::AbsView as AbstractMut>::Out;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Iter::Packed(iter) => iter.next(),
            Iter::NonPacked(iter) => iter.next(),
        }
    }
}

impl<T: IntoAbstract> IntoIter for T {
    type IntoIter = Iter<Self>;
    type IntoParIter = ParIter<Self>;
    fn iter(self) -> Self::IntoIter {
        use super::Len;

        let mut type_ids = Vec::new();
        T::add_type_id(&mut type_ids);
        type_ids.sort_unstable();

        let len = self.indices(&type_ids);

        match len {
            Len::Packed(end) => Iter::Packed(Packed {
                data: self.into_abstract(),
                current: 0,
                end,
            }),
            Len::Indices((indices, mut len)) => {
                if len == std::usize::MAX {
                    len = 0;
                }
                Iter::NonPacked(NonPacked {
                    data: self.into_abstract(),
                    indices,
                    current: 0,
                    end: len,
                })
            }
        }
    }
    fn par_iter(self) -> Self::IntoParIter {
        match self.iter() {
            Iter::Packed(iter) => ParIter::Packed(ParPacked(iter)),
            Iter::NonPacked(iter) => ParIter::NonPacked(ParNonPacked(iter)),
        }
    }
}

impl<T: IntoIter> IntoIter for (T,) {
    type IntoIter = T::IntoIter;
    type IntoParIter = T::IntoParIter;
    fn iter(self) -> Self::IntoIter {
        self.0.iter()
    }
    fn par_iter(self) -> Self::IntoParIter {
        self.0.par_iter()
    }
}

/// Packed iterator over components.
///
/// Packed iterators are the fastest but are limited to components packed together.
pub struct Packed<T: IntoAbstract> {
    data: T::AbsView,
    current: usize,
    end: usize,
}

impl<T: IntoAbstract> Packed<T> {
    /// Transform the iterator into a chunk iterator, returning multiple items.
    ///
    /// Chunk will return a smaller slice at the end if `size` does not divide exactly the length.
    pub fn into_chunk(self, size: usize) -> Chunk<T> {
        Chunk {
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
    pub fn into_chunk_exact(self, size: usize) -> ChunkExact<T> {
        ChunkExact {
            data: self.data,
            current: self.current,
            end: self.end,
            step: size,
        }
    }
}

impl<T: IntoAbstract> Iterator for Packed<T>
where
    T::AbsView: AbstractMut,
{
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

impl<T: IntoAbstract> DoubleEndedIterator for Packed<T>
where
    T::AbsView: AbstractMut,
{
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

impl<T: IntoAbstract> ExactSizeIterator for Packed<T>
where
    T::AbsView: AbstractMut,
{
    fn len(&self) -> usize {
        self.end - self.current
    }
}

/// Non packed iterator over multiple components.
pub struct NonPacked<T: IntoAbstract> {
    data: T::AbsView,
    pub(crate) indices: *const usize,
    pub(crate) current: usize,
    pub(crate) end: usize,
}

unsafe impl<T: IntoAbstract> Send for NonPacked<T> {}

impl<T: IntoAbstract> NonPacked<T>
where
    T::AbsView: AbstractMut,
{
    // Private version of clone, users should not be able to clone any iterators
    fn clone(&self) -> Self {
        NonPacked {
            data: self.data.clone(),
            indices: self.indices,
            current: self.current,
            end: self.end,
        }
    }
}

impl<T: IntoAbstract> Iterator for NonPacked<T>
where
    T::AbsView: AbstractMut,
{
    type Item = <T::AbsView as AbstractMut>::Out;
    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.end {
            // SAFE at this point there are no mutable reference to sparse or dense
            // and self.indices can't access out of bounds
            let index: usize = unsafe { std::ptr::read(self.indices.add(self.current)) };
            self.current += 1;
            // SAFE the index is valid and the iterator can only be created where the lifetime is valid
            if let Some(item) = unsafe { self.data.abs_get(index) } {
                return Some(item);
            } else {
                continue;
            }
        }
        None
    }
}

/// Chunk iterator over components.
///
/// Returns slices and not single elements.
///
/// The last chunk's length will be smaller than `size` if `size` does not divide the iterator's length perfectly.
pub struct Chunk<T: IntoAbstract> {
    data: T::AbsView,
    current: usize,
    end: usize,
    step: usize,
}

impl<T: IntoAbstract> Iterator for Chunk<T>
where
    T::AbsView: AbstractMut,
{
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

/// Chunk iterator over components.
///
/// Returns slices and not single elements.
///
/// The slices length will always by the same. To get the remaining elements (if any) use [remainder].
/// 
/// [remainder]: struct.ChunkExact.html#method.remainder
pub struct ChunkExact<T: IntoAbstract> {
    data: T::AbsView,
    current: usize,
    end: usize,
    step: usize,
}

impl<T: IntoAbstract> ChunkExact<T>
where
    T::AbsView: AbstractMut,
{
    /// Returns the items at the end of the slice.
    ///
    /// Will always return a slice smaller than `size`.
    pub fn remainder(&mut self) -> <T::AbsView as AbstractMut>::Slice {
        let remainder = std::cmp::min(self.end - self.current, self.end % self.step);
        let old_end = self.end;
        self.end -= remainder;
        unsafe { self.data.get_data_slice(self.end..old_end) }
    }
}

impl<T: IntoAbstract> Iterator for ChunkExact<T>
where
    T::AbsView: AbstractMut,
{
    type Item = <T::AbsView as AbstractMut>::Slice;
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        if current + self.step <= self.end {
            self.current += self.step;
            Some(unsafe { self.data.get_data_slice(current..self.current) })
        } else {
            None
        }
    }
}

/// Parallel iterator over components.
///
/// This enum allows to abstract away what kind of iterator you really get. That doesn't mean the performance will suffer.
pub enum ParIter<T: IntoAbstract> {
    Packed(ParPacked<T>),
    NonPacked(ParNonPacked<T>),
}

impl<T: IntoAbstract> ParallelIterator for ParIter<T>
where
    T::AbsView: AbstractMut,
    <T::AbsView as AbstractMut>::Out: Send,
{
    type Item = <T::AbsView as AbstractMut>::Out;
    fn drive_unindexed<Cons>(self, consumer: Cons) -> Cons::Result
    where
        Cons: UnindexedConsumer<Self::Item>,
    {
        match self {
            ParIter::Packed(iter) => bridge_producer_consumer(iter.0.len(), iter.0, consumer),
            ParIter::NonPacked(iter) => bridge_unindexed(iter.0, consumer),
        }
    }
}

/// Parallel iterator over components.
///
/// Packed owned iterators are fast but are limited to components packed together.
pub struct ParPacked<T: IntoAbstract>(Packed<T>);

impl<T: IntoAbstract> ParPacked<T> {
    /// Trasnform this parallel iterator into its sequential version.
    pub fn into_seq(self) -> Packed<T> {
        self.0
    }
}

impl<T: IntoAbstract> Producer for Packed<T>
where
    T::AbsView: AbstractMut,
    <T::AbsView as AbstractMut>::Out: Send,
{
    type Item = <T::AbsView as AbstractMut>::Out;
    type IntoIter = Self;
    fn into_iter(self) -> Self::IntoIter {
        self
    }
    fn split_at(mut self, index: usize) -> (Self, Self) {
        let clone = Packed {
            data: self.data.clone(),
            current: self.current + index,
            end: self.end,
        };
        self.end = clone.current;
        (self, clone)
    }
}

impl<T: IntoAbstract> ParallelIterator for ParPacked<T>
where
    T::AbsView: AbstractMut,
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

/// Parallel non packed iterator over multiple components.
pub struct ParNonPacked<T: IntoAbstract>(NonPacked<T>);

impl<T: IntoAbstract> UnindexedProducer for NonPacked<T>
where
    T::AbsView: AbstractMut,
{
    type Item = <T::AbsView as AbstractMut>::Out;
    fn split(mut self) -> (Self, Option<Self>) {
        let len = self.end - self.current;
        if len >= 2 {
            let mut clone = self.clone();
            clone.current += len / 2;
            self.end = clone.current;
            (self, Some(clone))
        } else {
            (self, None)
        }
    }
    fn fold_with<Fold>(self, folder: Fold) -> Fold
    where
        Fold: Folder<Self::Item>,
    {
        folder.consume_iter(self)
    }
}
