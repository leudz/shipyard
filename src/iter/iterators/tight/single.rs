#[cfg(feature = "parallel")]
use super::IntoIterator;
use super::{
    AbstractMut, Chunk1, ChunkExact1, CurrentId, DoubleEndedShiperator, ExactSizeShiperator,
    IntoAbstract, Shiperator,
};
use crate::EntityId;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::Producer;

pub struct Tight1<T: IntoAbstract> {
    data: T::AbsView,
    current: usize,
    end: usize,
}

impl<T: IntoAbstract> Tight1<T> {
    pub(crate) fn new(data: T) -> Self {
        Tight1 {
            current: 0,
            end: data.len().unwrap_or(0),
            data: data.into_abstract(),
        }
    }
    pub fn into_chunk(self, step: usize) -> Chunk1<T> {
        Chunk1 {
            data: self.data,
            current: self.current,
            end: self.end,
            step,
        }
    }
    pub fn into_chunk_exact(self, step: usize) -> ChunkExact1<T> {
        ChunkExact1 {
            data: self.data,
            current: self.current,
            end: self.end,
            step,
        }
    }
}

impl<T: IntoAbstract> Shiperator for Tight1<T> {
    type Item = <T::AbsView as AbstractMut>::Out;

    fn first_pass(&mut self) -> Option<Self::Item> {
        let current = self.current;
        if current < self.end {
            self.current += 1;
            let data = unsafe { self.data.get_data(current) };
            Some(data)
        } else {
            None
        }
    }
    fn post_process(&mut self) {}
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.end - self.current;
        (len, Some(len))
    }
}

impl<T: IntoAbstract> CurrentId for Tight1<T> {
    type Id = EntityId;

    unsafe fn current_id(&self) -> Self::Id {
        self.data.id_at(self.current - 1)
    }
}

impl<T: IntoAbstract> ExactSizeShiperator for Tight1<T> {}

impl<T: IntoAbstract> DoubleEndedShiperator for Tight1<T> {
    fn first_pass_back(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            self.end -= 1;
            let data = unsafe { self.data.get_data(self.end) };
            Some(data)
        } else {
            None
        }
    }
}

#[cfg(feature = "parallel")]
impl<T: IntoAbstract> Producer for Tight1<T>
where
    <T::AbsView as AbstractMut>::Out: Send,
    T::AbsView: Clone + Send,
{
    type Item = <T::AbsView as AbstractMut>::Out;
    type IntoIter = IntoIterator<Self>;
    fn into_iter(self) -> Self::IntoIter {
        <Self as Shiperator>::into_iter(self)
    }
    fn split_at(mut self, index: usize) -> (Self, Self) {
        let clone = Tight1 {
            data: self.data.clone(),
            current: self.current + index,
            end: self.end,
        };
        self.end = clone.current;
        (self, clone)
    }
}
