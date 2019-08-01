use super::{AbstractMut, IntoAbstract, IntoIter};

// Packed iterators go from start to end without index lookup
// They only work in specific circumstances but are the fastest
pub struct Packed<T: IntoAbstract> {
    data: T::View,
    current: usize,
    end: usize,
}

impl<T: IntoAbstract> IntoIter for T {
    type IntoIter = Packed<Self>;
    fn into_iter(self) -> Self::IntoIter {
        Packed {
            end: self.indices().1,
            data: self.into_abstract(),
            current: 0,
        }
    }
}

impl<T: IntoAbstract> Iterator for Packed<T> {
    type Item = <T::View as AbstractMut>::Out;
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        if current < self.end {
            self.current += 1;
            // SAFE the index is valid and the iterator can only be created where the lifetime is valid
            Some(unsafe { self.data.get_data(current) })
        } else {
            None
        }
    }
}
