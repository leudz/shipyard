use super::{AbstractMut, IntoAbstract, TightWithId1, UpdateWithId1, WithIdFilter1};
use crate::entity::Key;

pub enum WithId1<T: IntoAbstract> {
    Tight(TightWithId1<T>),
    Update(UpdateWithId1<T>),
}

impl<T: IntoAbstract> WithId1<T> {
    pub fn filtered<
        P: FnMut(&(Key, <<T as IntoAbstract>::AbsView as AbstractMut>::Out)) -> bool,
    >(
        self,
        pred: P,
    ) -> WithIdFilter1<T, P> {
        match self {
            WithId1::Tight(iter) => WithIdFilter1::Tight(iter.filtered(pred)),
            WithId1::Update(iter) => WithIdFilter1::Update(iter.filtered(pred)),
        }
    }
}

impl<T: IntoAbstract> Iterator for WithId1<T> {
    type Item = (Key, <<T as IntoAbstract>::AbsView as AbstractMut>::Out);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            WithId1::Tight(iter) => iter.next(),
            WithId1::Update(iter) => iter.next(),
        }
    }
}
