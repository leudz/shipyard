use super::{AbstractMut, IntoAbstract, TightFilter1};
use crate::entity::Key;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{Folder, Producer, UnindexedProducer};

pub struct TightFilterWithId1<T: IntoAbstract, P>(pub(super) TightFilter1<T, P>);

impl<T: IntoAbstract, P: FnMut(&<T::AbsView as AbstractMut>::Out) -> bool> Iterator
    for TightFilterWithId1<T, P>
{
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.0.next() {
            let current = self.0.iter.current - 1;
            let id = unsafe { self.0.iter.data.id_at(current) };
            Some((id, item))
        } else {
            None
        }
    }
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract, P: Fn(&<T::AbsView as AbstractMut>::Out) -> bool + Send + Sync>
    UnindexedProducer for TightFilterWithId1<T, &P>
where
    <T::AbsView as AbstractMut>::Out: Send,
{
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    fn split(self) -> (Self, Option<Self>) {
        let len = self.0.iter.len();
        if len >= 2 {
            let pred = self.0.pred;
            let (first, second) = self.0.iter.split_at(len / 2);
            let first = TightFilterWithId1(TightFilter1 { iter: first, pred });
            let second = TightFilterWithId1(TightFilter1 { iter: second, pred });
            (first, Some(second))
        } else {
            (self, None)
        }
    }
    fn fold_with<F>(self, folder: F) -> F
    where
        F: Folder<Self::Item>,
    {
        folder.consume_iter(self.0.with_id())
    }
}
