use alloc::vec::Vec;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::slice;

/// A list keeping up to `N` items inline, moving all of them to the heap past that.
///
/// Grouped analysis and the workload executor collect a handful of items at a time,
/// this saves allocating for them.
/// Longer lists work the same, they're just allocated.\
/// Unused inline slots hold `T::default()`.
#[derive(Clone)]
pub(crate) enum ScratchVec<T, const N: usize> {
    /// The first `len` items are the list, the others are placeholders.
    Inline {
        items: [T; N],
        len: usize,
    },
    Heap(Vec<T>),
}

impl<T: Default, const N: usize> ScratchVec<T, N> {
    /// Returns an empty list.
    pub(crate) fn new() -> ScratchVec<T, N> {
        ScratchVec::Inline {
            items: core::array::from_fn(|_| T::default()),
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
            ScratchVec::Inline { .. } => self.spill(item),
            ScratchVec::Heap(heap) => heap.push(item),
        }
    }

    /// Moves the full inline items to the heap, followed by `item`.
    ///
    /// Lists rarely get there, keeping it apart keeps `push` small enough to inline.
    #[cold]
    #[inline(never)]
    fn spill(&mut self, item: T) {
        let ScratchVec::Inline { items, .. } = self else {
            unreachable!("only inline lists spill");
        };

        let mut heap = Vec::with_capacity(2 * N);
        heap.extend(items.iter_mut().map(core::mem::take));
        heap.push(item);

        *self = ScratchVec::Heap(heap);
    }
}

impl<T: Default, const N: usize> Default for ScratchVec<T, N> {
    fn default() -> ScratchVec<T, N> {
        ScratchVec::new()
    }
}

impl<T: Default, const N: usize> FromIterator<T> for ScratchVec<T, N> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> ScratchVec<T, N> {
        let mut list = ScratchVec::new();
        for item in iter {
            list.push(item);
        }

        list
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
