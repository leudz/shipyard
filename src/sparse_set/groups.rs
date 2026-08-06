use alloc::vec::Vec;
use core::{any::TypeId, ops::Index};

pub(crate) struct Groups {
    type_ids: Vec<TypeId>,
    offsets: Vec<usize>,
}

impl Groups {
    #[inline]
    pub(crate) fn new() -> Self {
        Groups {
            type_ids: Vec::new(),
            offsets: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    #[inline]
    pub(crate) fn add(&mut self, group: &[TypeId]) -> usize {
        let index = self.offsets.len();

        self.offsets.push(self.type_ids.len());
        self.type_ids.extend_from_slice(group);

        index
    }

    /// Returns the group at `index` without doing bounds checking.
    ///
    /// # Safety
    ///
    /// `index` must be less than the number of groups added to this collection.
    #[inline]
    pub(crate) unsafe fn get_unchecked(&self, index: usize) -> &[TypeId] {
        let start = unsafe { *self.offsets.get_unchecked(index) };
        let end = if index + 1 == self.offsets.len() {
            self.type_ids.len()
        } else {
            unsafe { *self.offsets.get_unchecked(index + 1) }
        };

        unsafe { self.type_ids.get_unchecked(start..end) }
    }
}

impl Index<usize> for Groups {
    type Output = [TypeId];

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        let start = self.offsets[index];
        let end = self
            .offsets
            .get(index + 1)
            .copied()
            .unwrap_or(self.type_ids.len());

        &self.type_ids[start..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct A;
    struct B;
    struct C;

    #[test]
    fn add_and_index_varying_length_groups() {
        let mut groups = Groups::new();
        let a = TypeId::of::<A>();
        let b = TypeId::of::<B>();
        let c = TypeId::of::<C>();

        assert!(groups.offsets.is_empty());

        let empty = groups.add(&[]);
        let single = groups.add(&[a]);
        let multiple = groups.add(&[a, b, c]);
        let second_empty = groups.add(&[]);
        let duplicate = groups.add(&[a]);

        assert_eq!(empty, 0);
        assert_eq!(single, 1);
        assert_eq!(multiple, 2);
        assert_eq!(second_empty, 3);
        assert_eq!(duplicate, 4);
        assert_eq!(&groups[empty], &[]);
        assert_eq!(&groups[single], &[a]);
        assert_eq!(&groups[multiple], &[a, b, c]);
        assert_eq!(&groups[second_empty], &[]);
        assert_eq!(&groups[duplicate], &[a]);
        assert_eq!(unsafe { groups.get_unchecked(duplicate) }, &[a]);
    }

    #[test]
    fn indices_remain_valid_after_additions() {
        let mut groups = Groups::new();
        let a = TypeId::of::<A>();
        let b = TypeId::of::<B>();

        let first = groups.add(&[a, b]);

        for _ in 0..128 {
            groups.add(&[b]);
        }

        assert_eq!(&groups[first], &[a, b]);
        assert_eq!(unsafe { groups.get_unchecked(first) }, &[a, b]);
    }
}
