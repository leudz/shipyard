use super::{AbstractMut, IntoAbstract, TightWithIdFilter1, UpdateWithIdFilter1};
use crate::entity::Key;

pub enum WithIdFilter1<T: IntoAbstract, P: FnMut(&(Key, <T::AbsView as AbstractMut>::Out)) -> bool>
{
    Tight(TightWithIdFilter1<T, P>),
    Update(UpdateWithIdFilter1<T, P>),
}

impl<T: IntoAbstract, P: FnMut(&(Key, <T::AbsView as AbstractMut>::Out)) -> bool> Iterator
    for WithIdFilter1<T, P>
{
    type Item = <TightWithIdFilter1<T, P> as Iterator>::Item;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            WithIdFilter1::Tight(iter) => iter.next(),
            WithIdFilter1::Update(iter) => iter.next(),
        }
    }
}
