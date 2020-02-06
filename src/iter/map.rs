use super::{CurrentId, Shiperator};

pub struct Map<I, F> {
    iter: I,
    f: F,
}

impl<I, F> Map<I, F> {
    pub(super) fn new(iter: I, f: F) -> Self {
        Map { iter, f }
    }
}

impl<I: Shiperator, R, F> Shiperator for Map<I, F>
where
    F: FnMut(I::Item) -> R,
{
    type Item = R;

    fn first_pass(&mut self) -> Option<Self::Item> {
        let item = self.iter.first_pass()?;
        self.iter.post_process();
        Some((self.f)(item))
    }
    fn post_process(&mut self) {}
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<I: CurrentId, R, F> CurrentId for Map<I, F>
where
    F: FnMut(I::Item) -> R,
{
    type Id = I::Id;

    unsafe fn current_id(&self) -> Self::Id {
        self.iter.current_id()
    }
}
