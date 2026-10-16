use crate::entity_id::EntityId;
use crate::iter::{Shiper, Shiperator, WithId};

#[allow(missing_docs)]
pub struct ParShiperator<S>(pub(crate) Shiper<S>);

impl<S> ParShiperator<S> {
    /// Returns the [`EntityId`] alongside the component(s).
    pub fn with_id(self) -> WithId<Self> {
        WithId(self)
    }
}

impl<S: Shiperator + Send + Clone> rayon::iter::plumbing::UnindexedProducer for Shiper<S> {
    type Item = S::Out;

    #[inline]
    fn split(mut self) -> (Self, Option<Self>) {
        // We only split the current segment, neither half moves to another segment.
        let Some((left_cursor, right_cursor)) = self.cursor.split() else {
            return (self, None);
        };

        // Both halves keep the plan, the current segment's state might come from it.
        // `self` keeps the first half without follow-ups, the second half gets all of them.
        let right_schedule = self
            .schedule
            .as_mut()
            .map(|schedule| schedule.split_follow_ups());
        self.cursor = left_cursor;

        let right = Shiper {
            shiperator: self.shiperator.clone(),
            cursor: right_cursor,
            schedule: right_schedule,
            is_exact_sized: self.is_exact_sized,
        };

        (self, Some(right))
    }

    #[inline]
    fn fold_with<F>(self, folder: F) -> F
    where
        F: rayon::iter::plumbing::Folder<Self::Item>,
    {
        folder.consume_iter(self)
    }
}

impl<S: Shiperator + Send + Clone> rayon::iter::ParallelIterator for WithId<ParShiperator<S>>
where
    S::Out: Send,
{
    type Item = (EntityId, S::Out);

    #[inline]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: rayon::iter::plumbing::UnindexedConsumer<Self::Item>,
    {
        let WithId(ParShiperator(producer)) = self;

        rayon::iter::plumbing::bridge_unindexed(WithId(producer), consumer)
    }
}

impl<S: Shiperator + Send + Clone> rayon::iter::ParallelIterator for ParShiperator<S>
where
    S::Out: Send,
{
    type Item = S::Out;

    #[inline]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: rayon::iter::plumbing::UnindexedConsumer<Self::Item>,
    {
        rayon::iter::plumbing::bridge_unindexed(self.0, consumer)
    }

    #[inline]
    fn opt_len(&self) -> Option<usize> {
        if self.0.is_exact_sized {
            self.0.size_hint().1
        } else {
            None
        }
    }
}
