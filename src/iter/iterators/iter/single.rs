use super::{
    AbstractMut, Chunk1, ChunkExact1, CurrentId, IntoAbstract, IntoIterator, Shiperator, Tight1,
    Update1,
};
use crate::EntityId;

/// Iterator over a single component.
pub enum Iter1<T: IntoAbstract> {
    Tight(Tight1<T>),
    Update(Update1<T>),
}

impl<T: IntoAbstract> Iter1<T> {
    /// Tries to return a chunk iterator over `step` component at a time.  
    /// If `step` doesn't divide the length perfectly, the last chunk will be smaller.  
    /// In case this iterator can't be turned into a chunk iterator it will be returned.
    pub fn into_chunk(self, step: usize) -> Result<Chunk1<T>, Self> {
        match self {
            Self::Tight(tight) => Ok(tight.into_chunk(step)),
            _ => Err(self),
        }
    }
    /// Tries to return a chunk iterator over `step` component at a time.  
    /// If `step` doesn't divide the length perfectly, the remaining elements can be fetched with `remainder`.
    /// In case this iterator can't be turned into a chunk iterator it will be returned.
    pub fn into_chunk_exact(self, step: usize) -> Result<ChunkExact1<T>, Self> {
        match self {
            Self::Tight(tight) => Ok(tight.into_chunk_exact(step)),
            _ => Err(self),
        }
    }
}

impl<T: IntoAbstract> Shiperator for Iter1<T> {
    type Item = <T::AbsView as AbstractMut>::Out;

    fn first_pass(&mut self) -> Option<Self::Item> {
        match self {
            Self::Tight(tight) => tight.first_pass(),
            Self::Update(update) => update.first_pass(),
        }
    }
    fn post_process(&mut self) {
        match self {
            Self::Tight(tight) => tight.post_process(),
            Self::Update(update) => update.post_process(),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Tight(tight) => tight.size_hint(),
            Self::Update(update) => update.size_hint(),
        }
    }
}

impl<T: IntoAbstract> CurrentId for Iter1<T> {
    type Id = EntityId;

    unsafe fn current_id(&self) -> Self::Id {
        match self {
            Self::Tight(tight) => tight.current_id(),
            Self::Update(update) => update.current_id(),
        }
    }
}

impl<I: IntoAbstract> core::iter::IntoIterator for Iter1<I> {
    type IntoIter = IntoIterator<Self>;
    type Item = <Self as Shiperator>::Item;
    fn into_iter(self) -> Self::IntoIter {
        IntoIterator(self)
    }
}

impl<T: IntoAbstract> Clone for Iter1<T>
where
    Tight1<T>: Clone,
    Update1<T>: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Iter1::Tight(tight) => Iter1::Tight(tight.clone()),
            Iter1::Update(update) => Iter1::Update(update.clone()),
        }
    }
}

impl<T: IntoAbstract> Copy for Iter1<T>
where
    Tight1<T>: Copy,
    Update1<T>: Copy,
{
}
