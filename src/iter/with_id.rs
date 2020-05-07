use super::{CurrentId, IntoIterator, Shiperator};

/// Shiperator yielding `EntityId` as well.
#[derive(Clone, Copy)]
pub struct WithId<I> {
    iter: I,
}

impl<I> WithId<I> {
    pub(super) fn new(iter: I) -> Self {
        WithId { iter }
    }
}

impl<I: CurrentId> Shiperator for WithId<I> {
    type Item = (I::Id, I::Item);

    fn first_pass(&mut self) -> Option<Self::Item> {
        let item = self.iter.first_pass()?;
        // SAFE first_pass is called before
        Some((unsafe { self.iter.current_id() }, item))
    }
    fn post_process(&mut self) {
        self.iter.post_process()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<I: CurrentId> CurrentId for WithId<I> {
    type Id = I::Id;

    unsafe fn current_id(&self) -> Self::Id {
        self.iter.current_id()
    }
}

impl<I: CurrentId> core::iter::IntoIterator for WithId<I> {
    type IntoIter = IntoIterator<Self>;
    type Item = <Self as Shiperator>::Item;
    fn into_iter(self) -> Self::IntoIter {
        IntoIterator(self)
    }
}
