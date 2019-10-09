use super::InnerShiperator;
use crate::entity::Key;

pub struct Filter<I, P> {
    pub(crate) iter: I,
    pub(crate) pred: P,
}

impl<I: InnerShiperator, P: FnMut(&I::Item) -> bool> Iterator for Filter<I, P> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let first = self.first_pass()?;
        self.post_process(first)
    }
}

impl<I: InnerShiperator, P: FnMut(&I::Item) -> bool> InnerShiperator for Filter<I, P> {
    type Item = I::Item;
    type Index = I::Index;
    fn first_pass(&mut self) -> Option<(Self::Index, Self::Item)> {
        while let Some(first) = self.iter.first_pass() {
            if (self.pred)(&first.1) {
                return Some(first);
            }
        }
        None
    }
    #[inline]
    fn post_process(&mut self, first: (Self::Index, Self::Item)) -> Option<Self::Item> {
        self.iter.post_process(first)
    }
    #[inline]
    fn last_id(&self) -> Key {
        self.iter.last_id()
    }
}
