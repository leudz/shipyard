use alloc::boxed::Box;
use alloc::vec;

#[derive(Clone, Eq, Hash, PartialEq)]
pub(super) enum StorageMask {
    Inline(u64),
    Heap(Box<[u64]>),
}

impl StorageMask {
    pub(super) fn new(storage_count: usize) -> Self {
        if storage_count <= u64::BITS as usize {
            Self::Inline(0)
        } else {
            Self::Heap(vec![0; storage_count.div_ceil(u64::BITS as usize)].into_boxed_slice())
        }
    }

    pub(super) fn zero_like(other: &Self) -> Self {
        match other {
            Self::Inline(_) => Self::Inline(0),
            Self::Heap(words) => Self::Heap(vec![0; words.len()].into_boxed_slice()),
        }
    }

    #[inline]
    pub(super) fn words(&self) -> &[u64] {
        match self {
            Self::Inline(word) => core::slice::from_ref(word),
            Self::Heap(words) => words,
        }
    }

    #[inline]
    fn words_mut(&mut self) -> &mut [u64] {
        match self {
            Self::Inline(word) => core::slice::from_mut(word),
            Self::Heap(words) => words,
        }
    }

    #[inline]
    pub(super) fn insert(&mut self, storage_index: usize) -> bool {
        let word_index = storage_index / u64::BITS as usize;
        let bit = 1u64 << (storage_index % u64::BITS as usize);
        let word = &mut self.words_mut()[word_index];
        let was_missing = *word & bit == 0;
        *word |= bit;
        was_missing
    }

    #[inline]
    pub(super) fn intersects(&self, other: &Self) -> bool {
        debug_assert_eq!(self.words().len(), other.words().len());
        self.words()
            .iter()
            .zip(other.words())
            .any(|(&left, &right)| left & right != 0)
    }

    pub(super) fn union_with(&mut self, other: &Self) {
        debug_assert_eq!(self.words().len(), other.words().len());
        for (word, &other_word) in self.words_mut().iter_mut().zip(other.words()) {
            *word |= other_word;
        }
    }

    pub(super) fn intersect_with(&mut self, other: &Self) {
        debug_assert_eq!(self.words().len(), other.words().len());
        for (word, &other_word) in self.words_mut().iter_mut().zip(other.words()) {
            *word &= other_word;
        }
    }

    pub(super) fn difference_with(&mut self, other: &Self) {
        debug_assert_eq!(self.words().len(), other.words().len());
        for (word, &other_word) in self.words_mut().iter_mut().zip(other.words()) {
            *word &= !other_word;
        }
    }

    #[inline]
    pub(super) fn clear(&mut self) {
        self.words_mut().fill(0);
    }

    pub(super) fn first_difference(&self, other: &Self) -> Option<usize> {
        debug_assert_eq!(self.words().len(), other.words().len());
        self.words().iter().zip(other.words()).enumerate().find_map(
            |(word_index, (&word, &other_word))| {
                let difference = word & !other_word;

                (difference != 0)
                    .then(|| word_index * u64::BITS as usize + difference.trailing_zeros() as usize)
            },
        )
    }

    pub(super) fn indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.words()
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(word_index, mut word)| {
                core::iter::from_fn(move || {
                    if word == 0 {
                        return None;
                    }

                    let bit_index = word.trailing_zeros() as usize;
                    word &= word - 1;

                    Some(word_index * u64::BITS as usize + bit_index)
                })
            })
    }

    pub(super) fn count_ones(&self) -> usize {
        self.words()
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    #[inline]
    pub(super) fn is_subset_of(&self, other: &Self) -> bool {
        debug_assert_eq!(self.words().len(), other.words().len());
        self.words()
            .iter()
            .zip(other.words())
            .all(|(&left, &right)| left & !right == 0)
    }
}
