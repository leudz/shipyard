use crate::entity_id::EntityId;
use core::iter::Map;

pub struct WithId<I>(pub I);

pub trait IntoWithId {
    fn with_id(self) -> WithId<Self>
    where
        Self: Sized;
    #[allow(clippy::type_complexity)]
    fn ids(self) -> Map<WithId<Self>, fn(<WithId<Self> as Iterator>::Item) -> EntityId>
    where
        Self: Sized + Iterator,
        WithId<Self>: Iterator<Item = (EntityId, <Self as Iterator>::Item)>;
}

impl<I> IntoWithId for I
where
    I: Iterator,
    WithId<I>: Iterator<Item = (EntityId, <Self as Iterator>::Item)>,
{
    fn with_id(self) -> WithId<Self> {
        WithId(self)
    }
    #[allow(clippy::type_complexity)]
    fn ids(self) -> Map<WithId<Self>, fn(<WithId<Self> as Iterator>::Item) -> EntityId> {
        self.with_id().map(|(id, _)| id)
    }
}

pub trait LastId {
    /// ### Safety
    ///
    /// `Iterator::next` has to be called before it.
    unsafe fn last_id(&self) -> EntityId;
    /// ### Safety
    ///
    /// `DoubleEndedIterator::next_back` has to be called before it.
    unsafe fn last_id_back(&self) -> EntityId;
}

impl<I: Iterator + LastId> Iterator for WithId<I> {
    type Item = (EntityId, <I as Iterator>::Item);

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.0.next()?;

        Some((unsafe { self.0.last_id() }, item))
    }
}
