use super::{AbstractMut, IntoAbstract, TightWithId1};
use crate::storage::Key;

pub struct TightWithIdFilter1<T: IntoAbstract, P> {
    pub(super) iter: TightWithId1<T>,
    pub(super) pred: P,
}

impl<T: IntoAbstract, P: FnMut(&(Key, <T::AbsView as AbstractMut>::Out)) -> bool> Iterator
    for TightWithIdFilter1<T, P>
{
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    fn next(&mut self) -> Option<Self::Item> {
        for item in &mut self.iter {
            if (self.pred)(&item) {
                return Some(item);
            }
        }
        None
    }
}
