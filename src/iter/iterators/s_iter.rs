use super::{AbstractMut, Chunk1, ChunkExact1, InnerShiperator, IntoAbstract, Tight1, Update1};
use crate::entity::Key;

pub enum Iter1<T: IntoAbstract> {
    Tight(Tight1<T>),
    Update(Update1<T>),
}

impl<T: IntoAbstract> Iter1<T> {
    /// Tries to transform the iterator into a chunk iterator, returning `size` items at a time.
    /// If the component is packed with update pack the iterator is returned.
    ///
    /// Chunk will return a smaller slice at the end if `size` does not divide exactly the length.
    pub fn into_chunk(self, size: usize) -> Result<Chunk1<T>, Self> {
        match self {
            Iter1::Tight(iter) => Ok(iter.into_chunk(size)),
            Iter1::Update(_) => Err(self),
        }
    }
    /// Tries to transform the iterator into a chunk exact iterator, returning `size` items at a time.
    /// If the component is packed with update pack the iterator is returned.
    ///
    /// ChunkExact will always return a slice with the same length.
    ///
    /// To get the remaining items (if any) use the `remainder` method.
    pub fn into_chunk_exact(self, size: usize) -> Result<ChunkExact1<T>, Self> {
        match self {
            Iter1::Tight(iter) => Ok(iter.into_chunk_exact(size)),
            Iter1::Update(_) => Err(self),
        }
    }
}

impl<T: IntoAbstract> Iterator for Iter1<T> {
    type Item = <T::AbsView as AbstractMut>::Out;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Iter1::Tight(iter) => iter.next(),
            Iter1::Update(iter) => iter.next(),
        }
    }
    fn for_each<F>(self, f: F)
    where
        F: FnMut(Self::Item),
    {
        match self {
            Iter1::Tight(iter) => iter.for_each(f),
            Iter1::Update(iter) => iter.for_each(f),
        }
    }
    fn filter<P>(self, _: P) -> std::iter::Filter<Self, P>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        panic!("use filtered instead");
    }
}

impl<T: IntoAbstract> InnerShiperator for Iter1<T> {
    type Item = <Tight1<T> as InnerShiperator>::Item;
    type Index = <Tight1<T> as InnerShiperator>::Index;
    fn first_pass(&mut self) -> Option<(Self::Index, Self::Item)> {
        match self {
            Iter1::Tight(iter) => iter.first_pass(),
            Iter1::Update(iter) => iter.first_pass(),
        }
    }
    #[inline]
    fn post_process(&mut self, data: (Self::Index, Self::Item)) -> Option<Self::Item> {
        match self {
            Iter1::Tight(iter) => iter.post_process(data),
            Iter1::Update(iter) => iter.post_process(data),
        }
    }
    fn last_id(&self) -> Key {
        match self {
            Iter1::Tight(iter) => iter.last_id(),
            Iter1::Update(iter) => iter.last_id(),
        }
    }
}
