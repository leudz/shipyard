#[cfg(feature = "serde1")]
mod serde;

use core::num::NonZeroU64;

/// Handle to an entity.
// the id is 64 bits long
// <- 46 index -> <- 16 gen -> <- 2 meta ->
// a generation of !0 is used as a dead entity
// dead entities don't have any component
//
// shared entities have the generation !0 - 1
// inserted and modified component are flagged using metadata
#[derive(Clone, Copy, Eq)]
#[repr(transparent)]
pub struct EntityId(pub(super) NonZeroU64);

impl EntityId {
    // Number of bits used by the generation
    const GEN_LEN: u64 = 16;
    const META_LEN: u64 = 2;
    const INDEX_LEN: u64 = 64 - (Self::GEN_LEN + Self::META_LEN);
    const INDEX_MASK: u64 = !(!0 << Self::INDEX_LEN);
    const GEN_MASK: u64 = (!Self::INDEX_MASK) & (!Self::META_MASK);
    const META_MASK: u64 = !(!0 >> Self::META_LEN);
    const MAX_GEN: u64 = Self::GEN_MASK >> Self::INDEX_LEN;
    const SHARED: u64 = ((Self::GEN_MASK >> Self::INDEX_LEN) - 1) << Self::INDEX_LEN;
    const MODIFIED: u64 = 1 << (Self::INDEX_LEN + Self::GEN_LEN);
    const INSERTED: u64 = 2 << (Self::INDEX_LEN + Self::GEN_LEN);

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
    /// Modify the index.
    #[inline]
    pub(crate) fn set_index(&mut self, index: u64) {
        assert!(index < Self::INDEX_MASK);
        // SAFE never zero
        self.0 = unsafe {
            NonZeroU64::new_unchecked(
                (index + 1) | (self.0.get() & (Self::GEN_MASK | Self::META_MASK)),
            )
        }
    }
    /// Returns the generation part of the EntityId.
    #[inline]
    pub fn gen(self) -> u64 {
        (self.0.get() & Self::GEN_MASK) >> Self::INDEX_LEN
    }
    /// Increments the generation, returns Err if gen + 1 == gen::MAX().
    #[inline]
    pub(super) fn bump_gen(&mut self) -> Result<(), ()> {
        if self.gen() + 1 < Self::MAX_GEN - 1 {
            // SAFE never zero
            self.0 = unsafe {
                NonZeroU64::new_unchecked(
                    (self.0.get() & !Self::GEN_MASK) | ((self.gen() + 1) << Self::INDEX_LEN),
                )
            };
            Ok(())
        } else {
            Err(())
        }
    }
    #[inline]
    pub(crate) fn is_modified(self) -> bool {
        self.0.get() & Self::META_MASK == Self::MODIFIED
    }
    #[inline]
    pub(crate) fn set_modified(&mut self) {
        unsafe {
            self.0 = NonZeroU64::new_unchecked((self.0.get() & !Self::META_MASK) | Self::MODIFIED);
        }
    }
    #[inline]
    pub(crate) fn is_inserted(self) -> bool {
        self.0.get() & Self::META_MASK == Self::INSERTED
    }
    #[inline]
    pub(crate) fn set_inserted(&mut self) {
        unsafe {
            self.0 = NonZeroU64::new_unchecked((self.0.get() & !Self::META_MASK) | Self::INSERTED);
        }
    }
    #[inline]
    pub(crate) fn is_shared(self) -> bool {
        self.0.get() & Self::GEN_MASK == Self::SHARED
    }
    /// Make a new EntityId with the given index.
    #[inline]
    pub(crate) fn new(index: u64) -> Self {
        assert!(index < Self::INDEX_MASK);
        // SAFE never zero
        EntityId(unsafe { NonZeroU64::new_unchecked(index + 1) })
    }

    #[inline]
    pub(crate) fn new_shared(entity: Self) -> Self {
        // SAFE never zero
        EntityId(unsafe {
            NonZeroU64::new_unchecked(
                (((entity.0.get() & Self::GEN_MASK) >> Self::INDEX_LEN) + 1) | Self::SHARED,
            )
        })
    }

    pub(crate) fn new_from_parts(index: u64, gen: u16, meta: u8) -> Self {
        assert!(index < Self::INDEX_MASK);
        assert!(gen as u64 <= Self::max_gen());
        assert!(
            meta == 0
                || meta as u64 == Self::MODIFIED
                || meta as u64 == Self::INSERTED
                || meta as u64 == Self::SHARED
        );

        EntityId(unsafe {
            NonZeroU64::new_unchecked(
                (index + 1)
                    | (gen as u64) << Self::INDEX_LEN
                    | (meta as u64) << (Self::INDEX_LEN + Self::GEN_LEN),
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn zero() -> Self {
        EntityId(NonZeroU64::new(1).unwrap())
    }
    /// Returns a dead EntityId, it can be used as a null entity.
    #[inline]
    pub fn dead() -> Self {
        // SAFE not zero
        EntityId(unsafe { NonZeroU64::new_unchecked(!0) })
    }
    #[inline]
    pub(crate) fn bucket(self) -> usize {
        self.uindex() / crate::sparse_set::BUCKET_SIZE
    }
    #[inline]
    pub(crate) fn bucket_index(self) -> usize {
        self.uindex() % crate::sparse_set::BUCKET_SIZE
    }
    #[inline]
    pub(crate) fn shared_bucket(self) -> usize {
        self.uindex() / crate::sparse_set::SHARED_BUCKET_SIZE
    }
    #[inline]
    pub(crate) fn shared_bucket_index(self) -> usize {
        self.uindex() % crate::sparse_set::SHARED_BUCKET_SIZE
    }
    #[inline]
    pub(crate) fn max_index() -> u64 {
        Self::INDEX_MASK - 1
    }
    #[inline]
    pub(crate) fn max_gen() -> u64 {
        !0 >> (Self::INDEX_LEN + Self::META_LEN)
    }
    #[inline]
    pub(crate) fn is_dead(self) -> bool {
        self.0.get() & Self::GEN_MASK == Self::GEN_MASK
    }
    #[inline]
    pub(crate) fn copy_index(&mut self, other: EntityId) {
        unsafe {
            self.0 = NonZeroU64::new_unchecked(
                (self.0.get() & !Self::INDEX_MASK) | (other.0.get() & Self::INDEX_MASK),
            );
        }
    }
    #[inline]
    pub(crate) fn copy_gen(&mut self, other: EntityId) {
        unsafe {
            self.0 = NonZeroU64::new_unchecked(
                (self.0.get() & !Self::GEN_MASK) | (other.0.get() & Self::GEN_MASK),
            );
        }
    }
    #[inline]
    pub(crate) fn copy_index_gen(&mut self, other: EntityId) {
        unsafe {
            self.0 = NonZeroU64::new_unchecked(
                (self.0.get() & Self::META_MASK) | (other.0.get() & !Self::META_MASK),
            );
        }
    }
    #[inline]
    pub(crate) fn is_owned(self) -> bool {
        self.gen() < Self::MAX_GEN - 1
    }
    #[inline]
    pub(crate) fn clear_meta(&mut self) {
        unsafe {
            self.0 = NonZeroU64::new_unchecked(self.0.get() & !Self::META_MASK);
        }
    }
}

impl PartialEq for EntityId {
    fn eq(&self, other: &Self) -> bool {
        self.0.get() & !Self::META_MASK == other.0.get() & !Self::META_MASK
    }
}

impl PartialOrd for EntityId {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        let this = self.0.get() & !Self::META_MASK;
        let other = other.0.get() & !Self::META_MASK;

        this.partial_cmp(&other)
    }
}

impl Ord for EntityId {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let this = self.0.get() & !Self::META_MASK;
        let other = other.0.get() & !Self::META_MASK;

        this.cmp(&other)
    }
}

impl core::hash::Hash for EntityId {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
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
