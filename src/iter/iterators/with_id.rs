use super::InnerShiperator;
use crate::storage::Key;

pub struct WithId<I>(pub(crate) I);

impl<I: InnerShiperator> Iterator for WithId<I> {
    type Item = (Key, I::Item);
    fn next(&mut self) -> Option<Self::Item> {
        let first = self.first_pass()?;
        self.post_process(first)
    }
}

impl<I: InnerShiperator> InnerShiperator for WithId<I> {
    type Item = (Key, I::Item);
    type Index = ();
    fn first_pass(&mut self) -> Option<(Self::Index, Self::Item)> {
        let first = self.0.first_pass()?;
        Some(((), (self.last_id(), self.0.post_process(first)?)))
    }
    #[inline]
    fn post_process(&mut self, (_, item): (Self::Index, Self::Item)) -> Option<Self::Item> {
        Some(item)
    }
    #[inline]
    fn last_id(&self) -> Key {
        self.0.last_id()
    }
}
