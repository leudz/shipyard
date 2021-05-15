use crate::entity_id::EntityId;
use core::iter::Map;

#[allow(missing_docs)]
pub struct WithId<I>(pub I);

/// Creates iterator returning [`EntityId`].
///
/// [`EntityId`]: crate::entity_id::EntityId
pub trait IntoWithId {
    /// Makes the iterator return the [`EntityId`] in addition to the component.
    ///
    /// [`EntityId`]: crate::entity_id::EntityId
    fn with_id(self) -> WithId<Self>
    where
        Self: Sized;
    /// Makes the iterator only return the [`EntityId`] of each component.
    ///
    /// [`EntityId`]: crate::entity_id::EntityId
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

/// Can be used as bound for iterator that can use [`with_id`].
///
/// [`with_id`]: IntoWithId::with_id()
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
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
