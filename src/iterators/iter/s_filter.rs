use super::{AbstractMut, FilterWithId1, IntoAbstract, Tight1, TightFilter1, UpdateFilter1};

pub enum Filter1<T: IntoAbstract, P> {
    Tight(TightFilter1<T, P>),
    Update(UpdateFilter1<T, P>),
}

impl<T: IntoAbstract, P: FnMut(&<<T as IntoAbstract>::AbsView as AbstractMut>::Out) -> bool>
    Filter1<T, P>
{
    pub fn with_id(self) -> FilterWithId1<T, P> {
        match self {
            Filter1::Tight(iter) => FilterWithId1::Tight(iter.with_id()),
            Filter1::Update(iter) => FilterWithId1::Update(iter.with_id()),
        }
    }
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
