use super::abstract_mut::FastAbstractMut;
use super::chunk::FastChunk;
use super::chunk_exact::FastChunkExact;
use super::loose::FastLoose;
use super::mixed::FastMixed;
use super::tight::FastTight;
use crate::iter::with_id::LastId;
use crate::storage::EntityId;

pub enum FastIter<Storage> {
    Tight(FastTight<Storage>),
    Loose(FastLoose<Storage>),
    Mixed(FastMixed<Storage>),
}

impl<Storage: FastAbstractMut> FastIter<Storage> {
    pub fn into_chunk(self, step: usize) -> Result<FastChunk<Storage>, Self> {
        match self {
            FastIter::Tight(tight) => Ok(tight.into_chunk(step)),
            FastIter::Loose(_) => Err(self),
            FastIter::Mixed(_) => Err(self),
        }
    }
    pub fn into_chunk_exact(self, step: usize) -> Result<FastChunkExact<Storage>, Self> {
        match self {
            FastIter::Tight(tight) => Ok(tight.into_chunk_exact(step)),
            FastIter::Loose(_) => Err(self),
            FastIter::Mixed(_) => Err(self),
        }
    }
}

impl<Storage: FastAbstractMut> Iterator for FastIter<Storage>
where
    Storage::Index: Clone,
{
    type Item = <Storage as FastAbstractMut>::Out;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            FastIter::Tight(tight) => tight.next(),
            FastIter::Loose(loose) => loose.next(),
            FastIter::Mixed(mixed) => mixed.next(),
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            FastIter::Tight(tight) => tight.size_hint(),
            FastIter::Loose(loose) => loose.size_hint(),
            FastIter::Mixed(mixed) => mixed.size_hint(),
        }
    }
    #[inline]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        match self {
            FastIter::Tight(tight) => tight.fold(init, f),
            FastIter::Loose(loose) => loose.fold(init, f),
            FastIter::Mixed(mixed) => mixed.fold(init, f),
        }
    }
}

impl<Storage: FastAbstractMut> DoubleEndedIterator for FastIter<Storage>
where
    Storage::Index: Clone,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            FastIter::Tight(tight) => tight.next_back(),
            FastIter::Loose(loose) => loose.next_back(),
            FastIter::Mixed(mixed) => mixed.next_back(),
        }
    }
    #[inline]
    fn rfold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        match self {
            FastIter::Tight(tight) => tight.rfold(init, f),
            FastIter::Loose(loose) => loose.rfold(init, f),
            FastIter::Mixed(mixed) => mixed.rfold(init, f),
        }
    }
}

impl<Storage: FastAbstractMut> LastId for FastIter<Storage> {
    #[inline]
    unsafe fn last_id(&self) -> EntityId {
        match self {
            FastIter::Tight(tight) => tight.last_id(),
            FastIter::Loose(loose) => loose.last_id(),
            FastIter::Mixed(mixed) => mixed.last_id(),
        }
    }
    #[inline]
    unsafe fn last_id_back(&self) -> EntityId {
        match self {
            FastIter::Tight(tight) => tight.last_id_back(),
            FastIter::Loose(loose) => loose.last_id_back(),
            FastIter::Mixed(mixed) => mixed.last_id_back(),
        }
    }
}
