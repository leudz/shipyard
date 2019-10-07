use super::{AbstractMut, IntoAbstract, Tight1, TightFilterWithId1};

pub struct TightFilter1<T: IntoAbstract, P> {
    pub(super) iter: Tight1<T>,
    pub(super) pred: P,
}

impl<T: IntoAbstract, P> TightFilter1<T, P> {
    pub fn with_id(self) -> TightFilterWithId1<T, P> {
        TightFilterWithId1(self)
    }
}

impl<T: IntoAbstract, P: FnMut(&<T::AbsView as AbstractMut>::Out) -> bool> Iterator
    for TightFilter1<T, P>
{
    type Item = <Tight1<T> as Iterator>::Item;
    fn next(&mut self) -> Option<Self::Item> {
        for item in &mut self.iter {
            if (self.pred)(&item) {
                return Some(item);
            }
        }
        None
    }
}
