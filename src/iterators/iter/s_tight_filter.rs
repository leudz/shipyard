use crate::iterators::{AbstractMut, IntoAbstract, Tight1};

pub struct TightFilter1<T: IntoAbstract, P: FnMut(&<T::AbsView as AbstractMut>::Out) -> bool> {
    pub(super) iter: Tight1<T>,
    pub(super) pred: P,
}

impl<T: IntoAbstract, P: FnMut(&<T::AbsView as AbstractMut>::Out) -> bool> Iterator
    for TightFilter1<T, P>
{
    type Item = <T::AbsView as AbstractMut>::Out;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.iter.next() {
            if (self.pred)(&item) {
                Some(item)
            } else {
                None
            }
        } else {
            None
        }
    }
}
