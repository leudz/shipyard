use super::{AbstractMut, Chunk1, ChunkExact1, CurrentId, IntoAbstract, Shiperator};
use crate::EntityId;

pub struct Tight1<T: IntoAbstract> {
    pub(crate) data: T::AbsView,
    pub(crate) current: usize,
    pub(crate) end: usize,
}

impl<T: IntoAbstract> Tight1<T> {
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
    fn post_process(&mut self, item: Self::Item) -> Self::Item {
        item
    }
}

impl<T: IntoAbstract> CurrentId for Tight1<T> {
    type Id = EntityId;

    unsafe fn current_id(&self) -> Self::Id {
        self.data.id_at(self.current - 1)
    }
}
