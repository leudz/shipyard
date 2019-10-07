use super::{AbstractMut, IntoAbstract, UpdateFilter1};
use crate::entity::Key;

pub struct UpdateFilterWithId1<T: IntoAbstract, P>(pub(super) UpdateFilter1<T, P>);

impl<T: IntoAbstract, P: FnMut(&<T::AbsView as AbstractMut>::Out) -> bool> Iterator
    for UpdateFilterWithId1<T, P>
{
    type Item = (Key, <T::AbsView as AbstractMut>::Out);
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.0.next() {
            let id = self.0.last_id;
            Some((id, item))
        } else {
            None
        }
    }
}
