use super::abstract_mut::FastAbstractMut;

pub struct FastChunkExact<Storage> {
    pub(super) storage: Storage,
    pub(super) current: usize,
    pub(super) end: usize,
    pub(super) step: usize,
}

impl<Storage: FastAbstractMut> FastChunkExact<Storage> {
    pub fn remainder(&mut self) -> Storage::Slice {
        let remainder = core::cmp::min(self.end - self.current, self.end % self.step);
        let old_end = self.end;
        self.end -= remainder;
        // SAFE we checked for OOB and the lifetime is ok
        unsafe { self.storage.get_data_slice(self.end..old_end) }
    }
}

impl<Storage: FastAbstractMut> Iterator for FastChunkExact<Storage> {
    type Item = Storage::Slice;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.current + self.step < self.end {
            self.current += self.step;

            Some(unsafe {
                FastAbstractMut::get_data_slice(
                    &self.storage,
                    self.current - self.step..self.current,
                )
            })
        } else {
            None
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let exact = (self.end - self.current) / self.step;

        (exact, Some(exact))
    }
    #[inline]
    fn fold<B, F>(mut self, mut init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        while self.current + self.step < self.end {
            self.current += self.step;

            init = f(init, unsafe {
                FastAbstractMut::get_data_slice(
                    &self.storage,
                    self.current - self.step..self.current,
                )
            });
        }

        init
    }
}

impl<Storage: FastAbstractMut> ExactSizeIterator for FastChunkExact<Storage> {
    #[inline]
    fn len(&self) -> usize {
        (self.end - self.current) / self.step
    }
}

impl<Storage: FastAbstractMut> DoubleEndedIterator for FastChunkExact<Storage> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.current + self.step < self.end {
            self.end -= self.step;

            Some(unsafe {
                FastAbstractMut::get_data_slice(&self.storage, self.end..self.end + self.step)
            })
        } else {
            None
        }
    }
    #[inline]
    fn rfold<B, F>(mut self, mut init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        while self.current + self.step < self.end {
            self.end -= self.step;

            init = f(init, unsafe {
                FastAbstractMut::get_data_slice(&self.storage, self.end..self.end + self.step)
            });
        }

        init
    }
}
