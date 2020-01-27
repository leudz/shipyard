use super::{CurrentId, Shiperator};

pub struct Enumerate<I> {
    iter: I,
    count: usize,
}

impl<I> Enumerate<I> {
    pub(super) fn new(iter: I) -> Self {
        Enumerate { iter, count: 0 }
    }
}

impl<I: Shiperator> Shiperator for Enumerate<I> {
    type Item = (usize, I::Item);

    fn first_pass(&mut self) -> Option<Self::Item> {
        let item = self.iter.first_pass()?;
        let current = self.count;
        self.count += 1;
        Some((current, item))
    }
    fn post_process(&mut self, (current, item): Self::Item) -> Self::Item {
        (current, self.iter.post_process(item))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<I: CurrentId> CurrentId for Enumerate<I> {
    type Id = I::Id;

    unsafe fn current_id(&self) -> Self::Id {
        self.iter.current_id()
    }
}
