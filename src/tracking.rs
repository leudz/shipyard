mod iterator_wrapper;
mod tuple_track;

pub use iterator_wrapper::{Inserted, InsertedOrModified, Modified};
pub use tuple_track::TupleTrack;

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;

/// Trait implemented by all trackings.
pub trait Tracking: 'static + Sized + Sealed + Send + Sync {
    /// Associated numerical value that can be used to OR or AND trackings.
    const VALUE: u32;

    /// Makes sure we cannot borrow with less tracking than what was set in the Component impl
    /// this has to be in a doc test since it's a compile error
    ///
    /// ```compile_fail
    /// # use shipyard::{Component, track, View, World};
    /// #
    /// # struct Unit;
    /// # impl Component for Unit {
    /// #    type Tracking = track::Insertion;
    /// # }
    /// #
    /// # let mut world = World::new();
    /// #
    /// # world.borrow::<View<Unit, track::Modification>>();
    /// ```
    #[doc(hidden)]
    fn track_insertion() -> bool {
        Self::VALUE & 0b0001 != 0
    }

    /// Makes sure we cannot borrow with less tracking than what was set in the Component impl
    /// this has to be in a doc test since it's a compile error
    ///
    /// ```compile_fail
    /// # use shipyard::{Component, track, View, World};
    /// #
    /// # struct Unit;
    /// # impl Component for Unit {
    /// #    type Tracking = track::Modification;
    /// # }
    /// #
    /// # let mut world = World::new();
    /// #
    /// # world.borrow::<View<Unit, track::Insertion>>();
    /// ```
    #[doc(hidden)]
    fn track_modification() -> bool {
        Self::VALUE & 0b0010 != 0
    }

    /// Makes sure we cannot borrow with less tracking than what was set in the Component impl
    /// this has to be in a doc test since it's a compile error
    ///
    /// ```compile_fail
    /// # use shipyard::{Component, track, View, World};
    /// #
    /// # struct Unit;
    /// # impl Component for Unit {
    /// #    type Tracking = track::Deletion;
    /// # }
    /// #
    /// # let mut world = World::new();
    /// #
    /// # world.borrow::<View<Unit, track::Modification>>();
    /// ```
    #[doc(hidden)]
    fn track_deletion() -> bool {
        Self::VALUE & 0b0100 != 0
    }

    /// Makes sure we cannot borrow with less tracking than what was set in the Component impl
    /// this has to be in a doc test since it's a compile error
    ///
    /// ```compile_fail
    /// # use shipyard::{Component, track, View, World};
    /// #
    /// # struct Unit;
    /// # impl Component for Unit {
    /// #    type Tracking = track::Removal;
    /// # }
    /// #
    /// # let mut world = World::new();
    /// #
    /// # world.borrow::<View<Unit, track::Modification>>();
    /// ```
    #[doc(hidden)]
    fn track_removal() -> bool {
        Self::VALUE & 0b1000 != 0
    }

    #[doc(hidden)]
    fn name() -> &'static str;

    #[doc(hidden)]
    #[inline]
    fn is_inserted<T: Component>(
        _sparse_set: &SparseSet<T>,
        _entity: EntityId,
        _last: TrackingTimestamp,
        _current: TrackingTimestamp,
    ) -> bool {
        false
    }

    #[doc(hidden)]
    #[inline]
    fn is_modified<T: Component>(
        _sparse_set: &SparseSet<T>,
        _entity: EntityId,
        _last: TrackingTimestamp,
        _current: TrackingTimestamp,
    ) -> bool {
        false
    }

    #[doc(hidden)]
    #[inline]
    fn is_deleted<T: Component>(
        _sparse_set: &SparseSet<T>,
        _entity: EntityId,
        _last: TrackingTimestamp,
        _current: TrackingTimestamp,
    ) -> bool {
        false
    }

    #[doc(hidden)]
    #[inline]
    fn is_removed<T: Component>(
        _sparse_set: &SparseSet<T>,
        _entity: EntityId,
        _last: TrackingTimestamp,
        _current: TrackingTimestamp,
    ) -> bool {
        false
    }
}

/// Bound for tracking insertion.
pub trait InsertionTracking: Tracking {}
/// Bound for tracking modification.
pub trait ModificationTracking: Tracking {}
/// Bound for tracking removal.
pub trait RemovalTracking: Tracking + RemovalOrDeletionTracking {}
/// Bound for tracking deletion.
pub trait DeletionTracking: Tracking + RemovalOrDeletionTracking {}
/// Bound for tracking removal or deletion.
pub trait RemovalOrDeletionTracking: Tracking {
    #[doc(hidden)]
    #[allow(clippy::type_complexity)]
    fn removed_or_deleted<T: Component>(
        sparse_set: &SparseSet<T>,
    ) -> core::iter::Chain<
        core::iter::Map<
            core::slice::Iter<'_, (EntityId, TrackingTimestamp, T)>,
            fn(&(EntityId, TrackingTimestamp, T)) -> (EntityId, TrackingTimestamp),
        >,
        core::iter::Copied<core::slice::Iter<'_, (EntityId, TrackingTimestamp)>>,
    >;

    #[doc(hidden)]
    fn clear_all_removed_and_deleted<T: Component>(sparse_set: &mut SparseSet<T>);
    #[doc(hidden)]
    fn clear_all_removed_and_deleted_older_than_timestamp<T: Component>(
        sparse_set: &mut SparseSet<T>,
        _timestamp: TrackingTimestamp,
    );
}

#[inline]
pub(crate) fn map_deletion_data<T>(
    &(entity_id, timestamp, _): &(EntityId, TrackingTimestamp, T),
) -> (EntityId, TrackingTimestamp) {
    (entity_id, timestamp)
}

/// Timestamp used to clear tracking information.
#[derive(Clone, Copy, Debug)]
pub struct TrackingTimestamp(u64);

impl TrackingTimestamp {
    /// Returns a new [`TrackingTimestamp`] at the given tracking cycle.
    #[inline]
    pub fn new(now: u64) -> TrackingTimestamp {
        TrackingTimestamp(now)
    }

    #[inline]
    pub(crate) fn get(self) -> u64 {
        self.0
    }

    /// Returns `true` when the track timestamp is after the last time the system ran and before the current execution.
    ///
    /// This method should only be necessary for custom storages that want to implement tracking.
    #[inline]
    pub fn is_within(self, last: TrackingTimestamp, current: TrackingTimestamp) -> bool {
        let bounds = current.0.wrapping_sub(last.0.wrapping_add(1));
        let track = current.0.wrapping_sub(self.0);

        track <= bounds
    }

    /// Returns `true` when the track timestamp is within `u32::MAX / 2` cycles of `other`.
    ///
    /// This method should only be necessary for custom storages that want to implement tracking.
    #[inline]
    pub fn is_older_than(self, other: TrackingTimestamp) -> bool {
        other.0.wrapping_sub(1).wrapping_sub(self.0) < u64::MAX / 2
    }

    /// Returns the timesptamp the furthest from the given one.
    #[inline]
    pub fn furthest_from(self) -> TrackingTimestamp {
        TrackingTimestamp(self.0.wrapping_add(u64::MAX / 2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_within_bounds() {
        let tests = [
            (0, 0, 0, true),
            (5, 0, 10, true),
            (11, 0, 10, false),
            // check wrapping true
            (u64::MAX, u64::MAX - 1, 0, true),
            // check wrapping false
            (u64::MAX - 1, u64::MAX, 0, false),
            (1, 2, 0, false),
            // timestamp is equal to last
            (1, 1, 0, false),
            // timestamp is equal to now
            (1, 0, 1, true),
        ];

        for (timestamp, last, current, expected) in tests {
            assert_eq!(
                TrackingTimestamp::new(timestamp).is_within(
                    TrackingTimestamp::new(last),
                    TrackingTimestamp::new(current)
                ),
                expected,
                "t: {timestamp}, l: {last}, c: {current}"
            );
        }
    }

    #[test]
    fn is_older() {
        let tests = [
            (0, 0, false),
            (5, 10, true),
            (11, 10, false),
            // check wrapping true
            (u64::MAX, 0, true),
            // check wrapping false
            (0, u64::MAX, false),
            // barely within limit
            (0, u64::MAX / 2, true),
            // barely outside limit
            (0, u64::MAX / 2 + 1, false),
            // timestamp is equal to other
            (1, 1, false),
        ];

        for (timestamp, other, expected) in tests {
            assert_eq!(
                TrackingTimestamp::new(timestamp).is_older_than(TrackingTimestamp::new(other)),
                expected,
                "t: {timestamp}, o: {other}"
            );
        }
    }
}
