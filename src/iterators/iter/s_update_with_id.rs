use super::{AbstractMut, IntoAbstract, Update1, UpdateWithIdFilter1};
use crate::entity::Key;

pub struct UpdateWithId1<T: IntoAbstract>(pub(super) Update1<T>);

impl<T: IntoAbstract> UpdateWithId1<T> {
    pub fn filtered<
        P: FnMut(&(Key, <<T as IntoAbstract>::AbsView as AbstractMut>::Out)) -> bool,
    >(
        self,
        pred: P,
    ) -> UpdateWithIdFilter1<T, P> {
        UpdateWithIdFilter1 { iter: self, pred }
    }
}

impl<T: IntoAbstract> Iterator for UpdateWithId1<T> {
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|item| {
            let id = self.0.last_id;
            (id, item)
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<T: IntoAbstract> DoubleEndedIterator for UpdateWithId1<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|item| {
            let id = unsafe { self.0.data.id_at(self.0.end) };
            (id, item)
        })
    }
}

impl<T: IntoAbstract> ExactSizeIterator for UpdateWithId1<T> {
    fn len(&self) -> usize {
        self.0.len()
    }
}
