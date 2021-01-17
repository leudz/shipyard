use super::abstract_mut::FastAbstractMut;

pub struct FastChunk<Storage> {
    pub(super) storage: Storage,
    pub(super) current: usize,
    pub(super) end: usize,
    pub(super) step: usize,
}

impl<Storage: FastAbstractMut> Iterator for FastChunk<Storage> {
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
        } else if self.current < self.end {
            let result = Some(unsafe {
                FastAbstractMut::get_data_slice(&self.storage, self.current..self.end)
            });

            self.current = self.end;

            result
        } else {
            None
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let exact = (self.end - self.current + self.step - 1) / self.step;

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

        if self.current < self.end {
            init = f(init, unsafe {
                FastAbstractMut::get_data_slice(&self.storage, self.current..self.end)
            });

            self.current = self.end;
        }

        init
    }
}

impl<Storage: FastAbstractMut> ExactSizeIterator for FastChunk<Storage> {
    #[inline]
    fn len(&self) -> usize {
        (self.end - self.current + self.step - 1) / self.step
    }
}

impl<Storage: FastAbstractMut> DoubleEndedIterator for FastChunk<Storage> {
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
