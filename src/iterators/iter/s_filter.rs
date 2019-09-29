use crate::iterators;
use iterators::{AbstractMut, IntoAbstract, Tight1, TightFilter1, UpdateFilter1};

pub enum Filter1<
    T: IntoAbstract,
    P: FnMut(&<<T as IntoAbstract>::AbsView as AbstractMut>::Out) -> bool,
> {
    Tight(TightFilter1<T, P>),
    Update(UpdateFilter1<T, P>),
}

impl<T: IntoAbstract, P: FnMut(&<<T as IntoAbstract>::AbsView as AbstractMut>::Out) -> bool>
    Iterator for Filter1<T, P>
{
    type Item = <Tight1<T> as Iterator>::Item;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Filter1::Tight(iter) => iter.next(),
            Filter1::Update(iter) => iter.next(),
        }
    }
}
