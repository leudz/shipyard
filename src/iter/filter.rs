use super::{CurrentId, IntoIterator, Shiperator};

/// Shiperator filtering all components with `pred`.
pub struct Filter<I, P> {
    iter: I,
    pred: P,
}

impl<I, P> Filter<I, P> {
    pub(super) fn new(iter: I, pred: P) -> Self {
        Filter { iter, pred }
    }
}

impl<I: Shiperator, P> Shiperator for Filter<I, P>
where
    P: FnMut(&I::Item) -> bool,
{
    type Item = I::Item;

    fn first_pass(&mut self) -> Option<Self::Item> {
        while let Some(item) = self.iter.first_pass() {
            if (self.pred)(&item) {
                return Some(item);
            }
        }
        None
    }
    fn post_process(&mut self) {
        self.iter.post_process()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }
}

impl<I: CurrentId, P> CurrentId for Filter<I, P>
where
    P: FnMut(&I::Item) -> bool,
{
    type Id = I::Id;

    unsafe fn current_id(&self) -> Self::Id {
        self.iter.current_id()
    }
}

impl<I: Shiperator, P> core::iter::IntoIterator for Filter<I, P>
where
    P: FnMut(&I::Item) -> bool,
{
    type IntoIter = IntoIterator<Self>;
    type Item = <Self as Shiperator>::Item;
    fn into_iter(self) -> Self::IntoIter {
        IntoIterator(self)
    }
}
