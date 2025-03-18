use crate::iter::{Shiperator, ShiperatorCaptain, ShiperatorSailor};

#[allow(missing_docs)]
pub struct ParShiperator<S>(pub(crate) Shiperator<S>);

impl<S: ShiperatorCaptain + ShiperatorSailor + Send + Clone>
    rayon::iter::plumbing::UnindexedProducer for Shiperator<S>
{
    type Item = S::Out;

    fn split(self) -> (Self, Option<Self>) {
        let follow_up_len = self.entities.follow_up_len();
        let remaining = self.end - self.start;

        let max_len = self.end - self.start + follow_up_len;
        if max_len <= 1 {
            return (self, None);
        }

        let new_end = self.start + (remaining / 2);

        let (entities, other_entities) = self.entities.split_at(follow_up_len / 2);

        (
            Shiperator {
                shiperator: self.shiperator.clone(),
                entities,
                is_exact_sized: self.is_exact_sized,
                start: self.start,
                end: new_end,
            },
            Some(Shiperator {
                shiperator: self.shiperator,
                entities: other_entities,
                is_exact_sized: self.is_exact_sized,
                start: new_end,
                end: self.end,
            }),
        )
    }

    fn fold_with<F>(self, folder: F) -> F
    where
        F: rayon::iter::plumbing::Folder<Self::Item>,
    {
        folder.consume_iter(self)
    }
}

impl<S: ShiperatorCaptain + ShiperatorSailor + Send + Clone> rayon::iter::ParallelIterator
    for ParShiperator<S>
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
