mod iterator_wrapper;
mod tuple_track;

pub use iterator_wrapper::{Inserted, InsertedOrModified, Modified};
pub use tuple_track::TupleTrack;

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use core::fmt;

/// With tracking being a const generic it's not possible to implement traits directly to them.
/// This type is the way to implement traits on tracking constants.
pub struct Track<const T: u32>(());

impl<const TRACK: u32> fmt::Debug for Track<TRACK> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match TRACK {
            0b0000 => "Untracked",
            0b0001 => "Insertion",
            0b0010 => "Modification",
            0b0100 => "Deletion",
            0b1000 => "Removal",
            0b0011 => "Insertion and Modification",
            0b0101 => "Insertion and Deletion",
            0b1001 => "Insertion and Removal",
            0b0110 => "Modification and Deletion",
            0b1010 => "Modification and Removal",
            0b1100 => "Deletion and Removal",
            0b0111 => "Insertion, Modification and Deletion",
            0b1011 => "Insertion, Modification and Removal",
            0b1101 => "Insertion, Deletion and Removal",
            0b1110 => "Modification, Deletion and Removal",
            0b1111 => "All",
            _ => unreachable!(),
        })
    }
}

pub(crate) fn tracking_fmt(tracking: u32) -> &'static str {
    match tracking {
        0b0000 => "Untracked",
        0b0001 => "Insertion",
        0b0010 => "Modification",
        0b0100 => "Deletion",
        0b1000 => "Removal",
        0b0011 => "Insertion and Modification",
        0b0101 => "Insertion and Deletion",
        0b1001 => "Insertion and Removal",
        0b0110 => "Modification and Deletion",
        0b1010 => "Modification and Removal",
        0b1100 => "Deletion and Removal",
        0b0111 => "Insertion, Modification and Deletion",
        0b1011 => "Insertion, Modification and Removal",
        0b1101 => "Insertion, Deletion and Removal",
        0b1110 => "Modification, Deletion and Removal",
        0b1111 => "All",
        _ => unreachable!(),
    }
}

/// Trait implemented by all trackings.
pub trait Tracking: 'static + Sized + Sealed + Send + Sync {
    #[doc(hidden)]
    #[inline]
    fn is_inserted<T: Component>(
        _sparse_set: &SparseSet<T>,
        _entity: EntityId,
        _last: u32,
        _current: u32,
    ) -> bool {
        false
    }

    #[doc(hidden)]
    #[inline]
    fn is_modified<T: Component>(
        _sparse_set: &SparseSet<T>,
        _entity: EntityId,
        _last: u32,
        _current: u32,
    ) -> bool {
        false
    }

    #[doc(hidden)]
    #[inline]
    fn is_deleted<T: Component>(
        _sparse_set: &SparseSet<T>,
        _entity: EntityId,
        _last: u32,
        _current: u32,
    ) -> bool {
        false
    }

    #[doc(hidden)]
    #[inline]
    fn is_removed<T: Component>(
        _sparse_set: &SparseSet<T>,
        _entity: EntityId,
        _last: u32,
        _current: u32,
    ) -> bool {
        false
    }

    #[doc(hidden)]
    #[inline]
    fn remove<T: Component>(
        sparse_set: &mut SparseSet<T>,
        entity: EntityId,
        _current: u32,
    ) -> Option<T> {
        sparse_set.actual_remove(entity)
    }

    #[doc(hidden)]
    #[inline]
    fn delete<T: Component>(
        sparse_set: &mut SparseSet<T>,
        entity: EntityId,
        _current: u32,
    ) -> bool {
        sparse_set.actual_remove(entity).is_some()
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
            core::slice::Iter<'_, (EntityId, u32, T)>,
            fn(&(EntityId, u32, T)) -> (EntityId, u32),
        >,
        core::iter::Copied<core::slice::Iter<'_, (EntityId, u32)>>,
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
pub(crate) fn is_track_within_bounds(timestamp: u32, last: u32, current: u32) -> bool {
    let bounds = current.wrapping_sub(last);
    let track = current.wrapping_sub(timestamp);

    track < bounds
}

#[inline]
pub(crate) fn map_deletion_data<T>(
    &(entity_id, timestamp, _): &(EntityId, u32, T),
) -> (EntityId, u32) {
    (entity_id, timestamp)
}

/// Timestamp used to clear tracking information.
#[derive(Clone, Copy)]
pub struct TrackingTimestamp(pub(crate) u32);
