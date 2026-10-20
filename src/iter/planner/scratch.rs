use alloc::vec::Vec;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::slice;

/// A list keeping up to `N` items inline, moving all of them to the heap past that.
///
/// Grouped analysis collects a handful of items at a time, this saves allocating for them.
/// Longer lists work the same, they're just allocated.
#[derive(Clone)]
pub(crate) enum ScratchVec<T, const N: usize> {
    /// The first `len` items are the list, the others are placeholders.
    Inline {
        items: [T; N],
        len: usize,
    },
    Heap(Vec<T>),
}

impl<T: Copy + Default, const N: usize> ScratchVec<T, N> {
    /// Returns an empty list.
    pub(crate) fn new() -> ScratchVec<T, N> {
        ScratchVec::Inline {
            items: [T::default(); N],
            len: 0,
        }
    }

    /// Appends `item` to the list.
    pub(crate) fn push(&mut self, item: T) {
        match self {
            ScratchVec::Inline { items, len } if *len < N => {
                items[*len] = item;
                *len += 1;
            }
            // The inline items are full, they move to the heap ahead of `item`.
            ScratchVec::Inline { items, .. } => {
                let mut heap = Vec::with_capacity(2 * N);
                heap.extend_from_slice(items);
                heap.push(item);

                *self = ScratchVec::Heap(heap);
            }
            ScratchVec::Heap(heap) => heap.push(item),
        }
    }
}

impl<T, const N: usize> Deref for ScratchVec<T, N> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        match self {
            ScratchVec::Inline { items, len } => &items[..*len],
            ScratchVec::Heap(heap) => heap,
        }
    }
}

impl<T, const N: usize> DerefMut for ScratchVec<T, N> {
    fn deref_mut(&mut self) -> &mut [T] {
        match self {
            ScratchVec::Inline { items, len } => &mut items[..*len],
            ScratchVec::Heap(heap) => heap,
        }
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a ScratchVec<T, N> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> slice::Iter<'a, T> {
        self.iter()
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for ScratchVec<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
