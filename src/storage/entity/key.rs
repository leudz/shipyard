use std::num::NonZeroU64;

/// A Key is a handle to an entity and has two parts, the index and the version.
/// The length of the version can change but the index will always be size_of::<usize>() * 8 - version_len.
/// Valid versions can't exceed version::MAX() - 1, version::MAX() being used as flag for dead entities.
#[doc(hidden)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Key(pub(super) NonZeroU64);

impl Key {
    // Number of bits used by the version
    const VERSION_LEN: u64 = 16;
    const INDEX_MASK: u64 = !0 >> Self::VERSION_LEN;
    const VERSION_MASK: u64 = !Self::INDEX_MASK;

    /// Returns the index part of the Key.
    #[inline]
    pub(crate) fn index(self) -> usize {
        ((self.0.get() & Self::INDEX_MASK) - 1) as usize
    }
    /// Returns the version part of the Key.
    #[inline]
    pub(crate) fn version(self) -> usize {
        ((self.0.get() & Self::VERSION_MASK) >> (0usize.count_zeros() as u64 - Self::VERSION_LEN))
            as usize
    }
    /// Make a new Key with the given index.
    #[inline]
    pub(crate) fn new(index: u64) -> Self {
        assert!(index < Self::INDEX_MASK);
        Key(unsafe { NonZeroU64::new_unchecked(index + 1) })
    }

    /// Make a new Key with the given version and index.
    #[inline]
    pub(crate) fn new_from_pair(version: u64, index: u64) -> Self {
        assert!(index < Self::INDEX_MASK);
        //TODO: assert version is the right length
        Key(unsafe {
            NonZeroU64::new_unchecked((index+1) | (((version)) << (64 - Self::VERSION_LEN)))
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
        Key(NonZeroU64::new(1).unwrap())
    }
    pub(crate) fn dead() -> Self {
        Key(unsafe { NonZeroU64::new_unchecked(std::u64::MAX) })
    }
}

#[cfg(any(feature = "serde", test))]
impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.version(), self.index())
    }
}

#[test]
fn key() {
    let mut key = Key::new(0);
    assert_eq!(key.index(), 0);
    assert_eq!(key.version(), 0);
    key.set_index(701);
    assert_eq!(key.index(), 701);
    assert_eq!(key.version(), 0);
    key.bump_version().unwrap();
    key.bump_version().unwrap();
    key.bump_version().unwrap();
    assert_eq!(key.index(), 701);
    assert_eq!(key.version(), 3);
    key.set_index(554);
    assert_eq!(key.index(), 554);
    assert_eq!(key.version(), 3);
}
