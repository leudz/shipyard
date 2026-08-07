use alloc::vec::Vec;
use core::{any::TypeId, ops::Index};

pub(crate) struct Groups {
    /// Nested list of [TypeId] making up groups
    type_ids: Vec<TypeId>,
    /// Start index of each group
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
    pub(crate) fn iter(&self) -> impl Iterator<Item = &[TypeId]> {
        (0..self.offsets.len()).map(|index| &self[index])
    }

    #[inline]
    pub(crate) fn add(&mut self, group: &[TypeId]) -> usize {
        let index = self.offsets.len();

        self.offsets.push(self.type_ids.len());
        self.type_ids.extend_from_slice(group);

        index
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
    }
}
