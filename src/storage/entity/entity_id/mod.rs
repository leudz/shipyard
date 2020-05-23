#[cfg(feature = "serde")]
mod serde;

use core::num::NonZeroU64;

/// Handle to an entity.
///
/// It has two parts, an index and a generation.  
#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct EntityId(pub(super) NonZeroU64);

impl EntityId {
    // Number of bits used by the generation
    pub(crate) const GEN_LEN: u64 = 16;
    pub(super) const INDEX_MASK: u64 = !0 >> Self::GEN_LEN;
    pub(super) const GEN_MASK: u64 = !Self::INDEX_MASK;

    /// Returns the index part of the EntityId.  
    /// ⚠️ You shouldn't use it to index a storage.
    #[inline]
    pub fn index(self) -> u64 {
        (self.0.get() & Self::INDEX_MASK) - 1
    }
    /// Returns the index part of the EntityId as an usize.  
    /// ⚠️ You shouldn't use it to index a storage.
    #[inline]
    pub fn uindex(self) -> usize {
        self.index() as usize
    }
    /// Returns the generation part of the EntityId.
    #[inline]
    pub fn gen(self) -> u64 {
        (self.0.get() & Self::GEN_MASK) >> (64 - Self::GEN_LEN)
    }
    /// Make a new EntityId with the given index.
    #[inline]
    pub(crate) fn new(index: u64) -> Self {
        assert!(index < Self::INDEX_MASK);
        // SAFE never zero
        EntityId(unsafe { NonZeroU64::new_unchecked(index + 1) })
    }

    /// Make a new EntityId with the given generation and index.
    #[cfg(feature = "serde")]
    #[inline]
    pub(crate) fn new_from_pair(index: u64, gen: u16) -> Self {
        assert!(index < Self::INDEX_MASK);
        // SAFE never zero
        EntityId(unsafe {
            NonZeroU64::new_unchecked((index + 1) | ((gen as u64) << (64 - Self::GEN_LEN)))
        })
    }

    /// Modify the index.
    #[cfg(not(test))]
    #[inline]
    pub(super) fn set_index(&mut self, index: u64) {
        assert!(index < Self::INDEX_MASK);
        // SAFE never zero
        self.0 = unsafe { NonZeroU64::new_unchecked((self.0.get() & Self::GEN_MASK) | (index + 1)) }
    }
    /// Modify the index.
    #[cfg(test)]
    pub(crate) fn set_index(&mut self, index: u64) {
        assert!(index + 1 <= Self::INDEX_MASK);
        // SAFE never zero
        self.0 = unsafe { NonZeroU64::new_unchecked((self.0.get() & Self::GEN_MASK) | (index + 1)) }
    }
    /// Increments the generation, returns Err if gen + 1 == gen::MAX().
    #[inline]
    pub(super) fn bump_gen(&mut self) -> Result<(), ()> {
        if self.0.get() < !(!0 >> (Self::GEN_LEN - 1)) {
            // SAFE never zero
            self.0 = unsafe {
                NonZeroU64::new_unchecked(
                    (self.index() + 1) | ((self.gen() + 1) << (64 - Self::GEN_LEN)),
                )
            };
            Ok(())
        } else {
            Err(())
        }
    }
    #[cfg(test)]
    pub(crate) fn zero() -> Self {
        EntityId(NonZeroU64::new(1).unwrap())
    }
    /// Returns a dead EntityId, it can be used as a null entity.
    pub fn dead() -> Self {
        // SAFE not zero
        EntityId(unsafe { NonZeroU64::new_unchecked(core::u64::MAX) })
    }
    pub(crate) fn bucket(self) -> usize {
        self.uindex() / crate::sparse_set::BUCKET_SIZE
    }
    pub(crate) fn bucket_index(self) -> usize {
        self.uindex() % crate::sparse_set::BUCKET_SIZE
    }
}

impl core::fmt::Debug for EntityId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "EntityId {{ index: {}, gen: {} }}",
            self.index(),
            self.gen()
        )
    }
}

#[test]
fn entity_id() {
    let mut entity_id = EntityId::new(0);
    assert_eq!(entity_id.index(), 0);
    assert_eq!(entity_id.gen(), 0);
    entity_id.set_index(701);
    assert_eq!(entity_id.index(), 701);
    assert_eq!(entity_id.gen(), 0);
    entity_id.bump_gen().unwrap();
    entity_id.bump_gen().unwrap();
    entity_id.bump_gen().unwrap();
    assert_eq!(entity_id.index(), 701);
    assert_eq!(entity_id.gen(), 3);
    entity_id.set_index(554);
    assert_eq!(entity_id.index(), 554);
    assert_eq!(entity_id.gen(), 3);
}
