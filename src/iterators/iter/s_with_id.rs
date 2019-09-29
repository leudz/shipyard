use crate::entity::Key;
use crate::iterators;
use iterators::{AbstractMut, IntoAbstract, TightWithId1, UpdateWithId1};

pub enum WithId1<T: IntoAbstract> {
    Tight(TightWithId1<T>),
    Update(UpdateWithId1<T>),
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
