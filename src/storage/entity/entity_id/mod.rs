#[cfg(feature = "serialization")]
mod serialization;

use std::num::NonZeroU64;

/// A Key is a handle to an entity and has two parts, the index and the version.
/// The index is 48 bits long and the version 16.
#[doc(hidden)]
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct EntityId(pub(super) NonZeroU64);

impl EntityId {
    // Number of bits used by the version
    const VERSION_LEN: u64 = 16;
    const INDEX_MASK: u64 = !0 >> Self::VERSION_LEN;
    const VERSION_MASK: u64 = !Self::INDEX_MASK;

    /// Returns the index part of the EntityId.
    #[inline]
    pub(crate) fn index(self) -> usize {
        ((self.0.get() & Self::INDEX_MASK) - 1) as usize
    }
    /// Returns the version part of the EntityId.
    #[inline]
    pub(crate) fn version(self) -> usize {
        ((self.0.get() & Self::VERSION_MASK) >> (64 - Self::VERSION_LEN)) as usize
    }
    /// Make a new EntityId with the given index.
    #[inline]
    pub(crate) fn new(index: u64) -> Self {
        assert!(index < Self::INDEX_MASK);
        EntityId(unsafe { NonZeroU64::new_unchecked(index + 1) })
    }

    /// Make a new EntityId with the given version and index.
    #[cfg(feature = "serialization")]
    #[inline]
    pub(crate) fn new_from_pair(index: u64, version: u16) -> Self {
        assert!(index < Self::INDEX_MASK);
        EntityId(unsafe {
            NonZeroU64::new_unchecked((index + 1) | ((version as u64) << (64 - Self::VERSION_LEN)))
        })
    }

    /// Modify the index.
    #[cfg(not(test))]
    #[inline]
    pub(super) fn set_index(&mut self, index: u64) {
        assert!(index < Self::INDEX_MASK);
        self.0 =
            unsafe { NonZeroU64::new_unchecked((self.0.get() & Self::VERSION_MASK) | (index + 1)) }
    }
    /// Modify the index.
    #[cfg(test)]
    pub(crate) fn set_index(&mut self, index: u64) {
        assert!(index + 1 <= Self::INDEX_MASK);
        self.0 =
            unsafe { NonZeroU64::new_unchecked((self.0.get() & Self::VERSION_MASK) | (index + 1)) }
    }
    /// Increments the version, returns Err if version + 1 == version::MAX().
    #[inline]
    pub(super) fn bump_version(&mut self) -> Result<(), ()> {
        if self.0.get() < !(!0 >> (Self::VERSION_LEN - 1)) {
            self.0 = unsafe {
                NonZeroU64::new_unchecked(
                    (self.index() + 1) as u64
                        | (((self.version() + 1) as u64) << (64 - Self::VERSION_LEN)),
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
    pub fn dead() -> Self {
        EntityId(unsafe { NonZeroU64::new_unchecked(std::u64::MAX) })
    }
}

impl std::fmt::Debug for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EntityId {{ index: {}, version: {} }}",
            self.index(),
            self.version()
        )
    }
}

#[test]
fn entity_id() {
    let mut entity_id = EntityId::new(0);
    assert_eq!(entity_id.index(), 0);
    assert_eq!(entity_id.version(), 0);
    entity_id.set_index(701);
    assert_eq!(entity_id.index(), 701);
    assert_eq!(entity_id.version(), 0);
    entity_id.bump_version().unwrap();
    entity_id.bump_version().unwrap();
    entity_id.bump_version().unwrap();
    assert_eq!(entity_id.index(), 701);
    assert_eq!(entity_id.version(), 3);
    entity_id.set_index(554);
    assert_eq!(entity_id.index(), 554);
    assert_eq!(entity_id.version(), 3);
}
