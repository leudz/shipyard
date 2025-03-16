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

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use alloc::vec;
//     use core::ptr::NonNull;
//     use rayon::iter::plumbing::Producer;

//     // This impl isn't used in any test, it's only here to have a shiperator type
//     impl ShiperatorOutput for () {
//         type Out = ();
//     }

//     // This impl isn't used in any test, it's only here to have a shiperator type
//     impl ShiperatorCaptain for () {
//         unsafe fn get_captain_data(&self, _index: usize) -> Self::Out {
//             unreachable!()
//         }

//         fn sail_time(&self) -> usize {
//             unreachable!()
//         }

//         fn is_exact_sized(&self) -> bool {
//             unreachable!()
//         }

//         fn unpick(&mut self) {
//             unreachable!()
//         }
//     }

//     // This impl isn't used in any test, it's only here to have a shiperator type
//     impl ShiperatorSailor for () {
//         type Index = ();

//         unsafe fn get_sailor_data(&self, _index: Self::Index) -> Self::Out {
//             unreachable!()
//         }

//         fn indices_of(&self, _entity_id: crate::EntityId, _index: usize) -> Option<Self::Index> {
//             unreachable!()
//         }

//         fn index_from_usize(_index: usize) -> Self::Index {
//             unreachable!()
//         }
//     }

//     #[test]
//     fn split_at() {
//         let tests = [
//             (0, 0, vec![], 0, 0, 0),
//             (0, 10, vec![], 0, 0, 10),
//             (0, 10, vec![], 10, 10, 0),
//             (1, 10, vec![], 0, 0, 9),
//             (1, 10, vec![], 9, 9, 0),
//             (5, 5, vec![], 0, 0, 0),
//             (0, 5, vec![5], 0, 0, 10),
//             (0, 5, vec![5], 10, 10, 0),
//             (0, 5, vec![5], 7, 7, 3),
//             (0, 5, vec![5], 3, 3, 7),
//             (0, 5, vec![2, 5], 7, 7, 5),
//             (0, 5, vec![2, 5], 3, 3, 9),
//             (0, 5, vec![1, 1, 1, 5], 7, 7, 6),
//         ];

//         for (start, end, follow_up_lens, split_point, expected_len_left, expected_len_right) in
//             tests
//         {
//             let shiperator = Shiperator {
//                 shiperator: (),
//                 entities: RawEntityIdAccess {
//                     // This pointer isn't used in this test
//                     ptr: NonNull::dangling(),
//                     follow_up_ptrs: follow_up_lens
//                         .into_iter()
//                         .map(|len| (NonNull::dangling(), len))
//                         .collect(),
//                 },
//                 is_exact_sized: true,
//                 start,
//                 end,
//             };

//             let (left, right) = shiperator.split_at(split_point);

//             assert_eq!(left.len(), expected_len_left);
//             assert_eq!(right.len(), expected_len_right);
//         }
//     }
// }
