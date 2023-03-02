use crate::atomic_refcell::SharedBorrow;
use crate::component::Unique;
use crate::tracking::is_track_within_bounds;
use crate::unique::UniqueStorage;
use core::fmt;
use core::ops::Deref;

/// Shared view over a unique component storage.
pub struct UniqueView<'a, T: Unique> {
    pub(crate) unique: &'a UniqueStorage<T>,
    pub(crate) borrow: Option<SharedBorrow<'a>>,
    pub(crate) all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) last_insertion: u32,
    pub(crate) last_modification: u32,
    pub(crate) current: u32,
}

impl<T: Unique> UniqueView<'_, T> {
    /// Duplicates the [`UniqueView`].
    #[allow(clippy::should_implement_trait)]
    #[inline]
    pub fn clone(unique: &Self) -> Self {
        UniqueView {
            unique: unique.unique,
            borrow: unique.borrow.clone(),
            all_borrow: unique.all_borrow.clone(),
            last_insertion: unique.last_insertion,
            last_modification: unique.last_modification,
            current: unique.current,
        }
    }
}

impl<T: Unique> UniqueView<'_, T> {
    /// Returns `true` if the component was inserted before the last [`clear_inserted`] call.  
    ///
    /// [`clear_inserted`]: UniqueViewMut::clear_inserted
    #[inline]
    pub fn is_inserted(&self) -> bool {
        is_track_within_bounds(self.unique.insert, self.last_insertion, self.current)
    }
    /// Returns `true` is the component was modified since the last [`clear_modified`] call.
    ///
    /// [`clear_modified`]: UniqueViewMut::clear_modified
    #[inline]
    pub fn is_modified(&self) -> bool {
        is_track_within_bounds(
            self.unique.modification,
            self.last_modification,
            self.current,
        )
    }
    /// Returns `true` if the component was inserted or modified since the last [`clear_inserted`] or [`clear_modified`] call.  
    ///
    /// [`clear_inserted`]: UniqueViewMut::clear_inserted
    /// [`clear_modified`]: UniqueViewMut::clear_modified
    #[inline]
    pub fn is_inserted_or_modified(&self) -> bool {
        self.is_inserted() || self.is_modified()
    }
}

impl<T: Unique> Deref for UniqueView<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.unique.value
    }
}

impl<T: Unique> AsRef<T> for UniqueView<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.unique.value
    }
}

impl<T: fmt::Debug + Unique> fmt::Debug for UniqueView<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.unique.value.fmt(f)
    }
}
