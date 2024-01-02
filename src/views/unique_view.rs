use crate::atomic_refcell::SharedBorrow;
use crate::component::Unique;
use crate::tracking::TrackingTimestamp;
use crate::unique::UniqueStorage;
use core::fmt;
use core::ops::Deref;

/// Shared view over a unique component storage.
pub struct UniqueView<'a, T: Unique> {
    pub(crate) unique: &'a UniqueStorage<T>,
    pub(crate) borrow: Option<SharedBorrow<'a>>,
    pub(crate) all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) last_insertion: TrackingTimestamp,
    pub(crate) last_modification: TrackingTimestamp,
    pub(crate) current: TrackingTimestamp,
}

impl<T: Unique> UniqueView<'_, T> {
    /// Duplicates the [`UniqueView`].
    ///
    /// [`Clone`] is not implemented to not conflict with `T::clone`.
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

impl<T: Unique> UniqueView<'_, T> {
    /// Returns `true` if the component was inserted before the last [`clear_inserted`] call.  
    ///
    /// [`clear_inserted`]: crate::UniqueViewMut::clear_inserted
    #[inline]
    pub fn is_inserted(&self) -> bool {
        self.unique
            .insert
            .is_within(self.last_insertion, self.current)
    }
    /// Returns `true` is the component was modified since the last [`clear_modified`] call.
    ///
    /// [`clear_modified`]: crate::UniqueViewMut::clear_modified
    #[inline]
    pub fn is_modified(&self) -> bool {
        self.unique
            .modification
            .is_within(self.last_modification, self.current)
    }
    /// Returns `true` if the component was inserted or modified since the last [`clear_inserted`] or [`clear_modified`] call.  
    ///
    /// [`clear_inserted`]: crate::UniqueViewMut::clear_inserted
    /// [`clear_modified`]: crate::UniqueViewMut::clear_modified
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
