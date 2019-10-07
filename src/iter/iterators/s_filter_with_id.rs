use super::{AbstractMut, IntoAbstract, TightFilterWithId1, UpdateFilterWithId1};

pub enum FilterWithId1<T: IntoAbstract, P> {
    Tight(TightFilterWithId1<T, P>),
    Update(UpdateFilterWithId1<T, P>),
}

impl<T: IntoAbstract, P: FnMut(&<<T as IntoAbstract>::AbsView as AbstractMut>::Out) -> bool>
    Iterator for FilterWithId1<T, P>
{
    type Item = <TightFilterWithId1<T, P> as Iterator>::Item;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            FilterWithId1::Tight(iter) => iter.next(),
            FilterWithId1::Update(iter) => iter.next(),
        }
    }
}
