use super::{AbstractMut, Chunk1, ChunkExact1, Filter1, IntoAbstract, Tight1, Update1, WithId1};

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
    pub fn filtered<P: FnMut(&<<T as IntoAbstract>::AbsView as AbstractMut>::Out) -> bool>(
        self,
        pred: P,
    ) -> Filter1<T, P> {
        match self {
            Iter1::Tight(iter) => Filter1::Tight(iter.filtered(pred)),
            Iter1::Update(iter) => Filter1::Update(iter.filtered(pred)),
        }
    }
    pub fn with_id(self) -> WithId1<T> {
        match self {
            Iter1::Tight(iter) => WithId1::Tight(iter.with_id()),
            Iter1::Update(iter) => WithId1::Update(iter.with_id()),
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
