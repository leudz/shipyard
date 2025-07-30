use crate::atomic_refcell::{ExclusiveBorrow, SharedBorrow};
use crate::component::Unique;
use crate::tracking::TrackingTimestamp;
use crate::unique::UniqueStorage;
use core::fmt;
use core::ops::{Deref, DerefMut};

/// Exclusive view over a unique component storage.
pub struct UniqueViewMut<'a, T: Unique> {
    pub(crate) unique: &'a mut UniqueStorage<T>,
    pub(crate) _borrow: Option<ExclusiveBorrow<'a>>,
    pub(crate) _all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) last_insertion: TrackingTimestamp,
    pub(crate) last_modification: TrackingTimestamp,
    pub(crate) current: TrackingTimestamp,
}

impl<T: Unique> UniqueViewMut<'_, T> {
    /// Returns `true` if the component was inserted before the last [`clear_inserted`] call.  
    ///
    /// [`clear_inserted`]: Self::clear_inserted
    #[inline]
    pub fn is_inserted(&self) -> bool {
        self.unique
            .insert
            .is_within(self.last_insertion, self.current)
    }
    /// Returns `true` if the component was modified since the last [`clear_modified`] call.  
    ///
    /// [`clear_modified`]: Self::clear_modified
    #[inline]
    pub fn is_modified(&self) -> bool {
        self.unique
            .modification
            .is_within(self.last_modification, self.current)
    }
    /// Returns `true` if the component was inserted or modified since the last [`clear_inserted`] or [`clear_modified`] call.  
    ///
    /// [`clear_inserted`]: Self::clear_inserted
    /// [`clear_modified`]: Self::clear_modified
    #[inline]
    pub fn is_inserted_or_modified(&self) -> bool {
        self.is_inserted() || self.is_modified()
    }
    /// Removes the *inserted* flag on the component of this storage.
    #[inline]
    pub fn clear_inserted(self) {
        self.unique.last_insert = self.current;
    }
    /// Removes the *modified* flag on the component of this storage.
    #[inline]
    pub fn clear_modified(self) {
        self.unique.last_modification = self.current;
    }
    /// Removes the *inserted* and *modified* flags on the component of this storage.
    #[inline]
    pub fn clear_inserted_and_modified(self) {
        self.unique.last_insert = self.current;
        self.unique.last_modification = self.current;
    }

    /// Replaces the timestamp starting the tracking time window for insertions.
    ///
    /// Tracking works based on a time window. From the last time the system ran (in workloads)
    /// or since the last clear.
    ///
    /// Sometimes this automatic time window isn't what you need.
    /// This can happen when you want to keep the same tracking information for multiple runs
    /// of the same system.
    ///
    /// For example if you interpolate movement between frames, you might run an interpolation workload
    /// multiple times but not change the [`World`](crate::World) during its execution.\
    /// In this case you want the same tracking information for all runs of this workload
    /// which would have disappeared using the automatic window.
    pub fn override_last_insertion(
        &mut self,
        new_timestamp: TrackingTimestamp,
    ) -> TrackingTimestamp {
        core::mem::replace(&mut self.last_insertion, new_timestamp)
    }

    /// Replaces the timestamp starting the tracking time window for modifications.
    ///
    /// Tracking works based on a time window. From the last time the system ran (in workloads)
    /// or since the last clear.
    ///
    /// Sometimes this automatic time window isn't what you need.
    /// This can happen when you want to keep the same tracking information for multiple runs
    /// of the same system.
    ///
    /// For example if you interpolate movement between frames, you might run an interpolation workload
    /// multiple times but not change the [`World`](crate::World) during its execution.\
    /// In this case you want the same tracking information for all runs of this workload
    /// which would have disappeared using the automatic window.
    pub fn override_last_modification(
        &mut self,
        new_timestamp: TrackingTimestamp,
    ) -> TrackingTimestamp {
        core::mem::replace(&mut self.last_modification, new_timestamp)
    }
}

impl<T: Unique + Clone> UniqueViewMut<'_, T> {
    /// Registers the function to clone this unique component.
    pub fn register_clone(&mut self) {
        self.unique.register_clone();
    }
}

impl<T: Unique> Deref for UniqueViewMut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.unique.value
    }
}

impl<T: Unique> DerefMut for UniqueViewMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.unique.modification = self.current;

        &mut self.unique.value
    }
}

impl<T: Unique> AsRef<T> for UniqueViewMut<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.unique.value
    }
}

impl<T: Unique> AsMut<T> for UniqueViewMut<'_, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self.unique.modification = self.current;

        &mut self.unique.value
    }
}

impl<T: fmt::Debug + Unique> fmt::Debug for UniqueViewMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.unique.value.fmt(f)
    }
}
