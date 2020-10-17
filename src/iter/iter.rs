use super::abstract_mut::AbstractMut;
use super::mixed::Mixed;
use super::tight::Tight;
use super::with_id::LastId;
use crate::storage::EntityId;

pub enum Iter<Storage> {
    Tight(Tight<Storage>),
    Mixed(Mixed<Storage>),
}

impl<Storage: AbstractMut> Iterator for Iter<Storage>
where
    <Storage as AbstractMut>::Index: Clone,
{
    type Item = Storage::Out;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Iter::Tight(tight) => tight.next(),
            Iter::Mixed(mixed) => mixed.next(),
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Iter::Tight(tight) => tight.size_hint(),
            Iter::Mixed(mixed) => mixed.size_hint(),
        }
    }
    #[inline]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        match self {
            Iter::Tight(tight) => tight.fold(init, f),
            Iter::Mixed(mixed) => mixed.fold(init, f),
        }
    }
}

impl<Storage: AbstractMut> DoubleEndedIterator for Iter<Storage>
where
    <Storage as AbstractMut>::Index: Clone,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Iter::Tight(tight) => tight.next_back(),
            Iter::Mixed(mixed) => mixed.next_back(),
        }
    }
    #[inline]
    fn rfold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        match self {
            Iter::Tight(tight) => tight.rfold(init, f),
            Iter::Mixed(mixed) => mixed.rfold(init, f),
        }
    }
}

impl<Storage: AbstractMut> LastId for Iter<Storage> {
    #[inline]
    unsafe fn last_id(&self) -> EntityId {
        match self {
            Iter::Tight(tight) => tight.last_id(),
            Iter::Mixed(mixed) => mixed.last_id(),
        }
    }
    #[inline]
    unsafe fn last_id_back(&self) -> EntityId {
        match self {
            Iter::Tight(tight) => tight.last_id_back(),
            Iter::Mixed(mixed) => mixed.last_id_back(),
        }
    }
}
