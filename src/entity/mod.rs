mod add_component;
mod view;

use crate::unknown_storage::UnknownStorage;
use std::any::TypeId;
use std::num::NonZeroU64;
pub use view::{EntitiesView, EntitiesViewMut};

/// A Key is a handle to an entity and has two parts, the index and the version.
/// The length of the version can change but the index will always be size_of::<usize>() * 8 - version_len.
/// Valid versions can't exceed version::MAX() - 1, version::MAX() being used as flag for dead entities.
#[doc(hidden)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Key(NonZeroU64);

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
    /// Modify the index.
    #[cfg(not(test))]
    #[inline]
    fn set_index(&mut self, index: u64) {
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
    fn bump_version(&mut self) -> Result<(), ()> {
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

/// Type used to borrow `Entities` mutably.
pub struct EntitiesMut;

/// Entities holds the Keys to all entities: living, removed and dead.
///
/// A living entity is an entity currently present, with or without component.
///
/// Removed and dead entities don't have any component.
///
/// The big difference is that removed ones can become alive again.
///
/// The life cycle of an entity looks like this:
///
/// Generation -> Deletion -> Dead\
///           ⬑----------↵
// An entity starts with a generation at 0, each removal will increase it by 1
// until version::MAX() where the entity is considered dead.
// Removed entities form a linked list inside the vector, using their index part to point to the next.
// Removed entities are added to one end and removed from the other.
// Dead entities are simply never added to the linked list.
pub struct Entities {
    data: Vec<Key>,
    list: Option<(usize, usize)>,
}

impl Default for Entities {
    fn default() -> Self {
        Entities {
            data: Vec::new(),
            list: None,
        }
    }
}

impl Entities {
    pub(crate) fn view(&self) -> EntitiesView {
        EntitiesView { data: &self.data }
    }
    pub(crate) fn view_mut(&mut self) -> EntitiesViewMut {
        EntitiesViewMut {
            data: &mut self.data,
            list: &mut self.list,
        }
    }
}

impl UnknownStorage for Entities {
    fn delete(&mut self, _entity: Key) -> &[TypeId] {
        unimplemented!()
    }
    fn unpack(&mut self, _entity: Key) {}
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
#[test]
fn entities() {
    let mut entities = Entities::default();

    let key00 = entities.view_mut().generate();
    let key10 = entities.view_mut().generate();

    assert_eq!(key00.index(), 0);
    assert_eq!(key00.version(), 0);
    assert_eq!(key10.index(), 1);
    assert_eq!(key10.version(), 0);

    assert!(entities.view_mut().delete_key(key00));
    assert!(!entities.view_mut().delete_key(key00));
    let key01 = entities.view_mut().generate();

    assert_eq!(key01.index(), 0);
    assert_eq!(key01.version(), 1);

    assert!(entities.view_mut().delete_key(key10));
    assert!(entities.view_mut().delete_key(key01));
    let key11 = entities.view_mut().generate();
    let key02 = entities.view_mut().generate();

    assert_eq!(key11.index(), 1);
    assert_eq!(key11.version(), 1);
    assert_eq!(key02.index(), 0);
    assert_eq!(key02.version(), 2);

    let last_key = Key(NonZeroU64::new(!(!0 >> 15) + 1).unwrap());
    entities.data[0] = last_key;
    assert!(entities.view_mut().delete_key(last_key));
    assert_eq!(entities.list, None);
    let dead = entities.view_mut().generate();
    assert_eq!(dead.index(), 2);
    assert_eq!(dead.version(), 0);
}
