mod iterator_wrapper;
mod tuple_track;

pub use iterator_wrapper::{Inserted, InsertedOrModified, Modified};
pub use tuple_track::TupleTrack;

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::track::{
    All, Deletion, DeletionAndRemoval, Insertion, InsertionAndDeletion,
    InsertionAndDeletionAndRemoval, InsertionAndModification, InsertionAndModificationAndDeletion,
    InsertionAndModificationAndRemoval, InsertionAndRemoval, Modification, ModificationAndDeletion,
    ModificationAndDeletionAndRemoval, ModificationAndRemoval, Removal, Untracked,
};
use core::fmt;

/// When tracking will be a const generic it will not be possible to implement traits directly on them.
/// This type will be the way to implement traits on tracking constants.
pub struct Track<T>(T);

impl fmt::Debug for Track<Untracked> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Untracked")
    }
}
impl fmt::Debug for Track<Insertion> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Insertion")
    }
}
impl fmt::Debug for Track<InsertionAndModification> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Insertion and Modification")
    }
}
impl fmt::Debug for Track<InsertionAndModificationAndDeletion> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Insertion, Modification and Deletion")
    }
}
impl fmt::Debug for Track<InsertionAndModificationAndRemoval> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Insertion, Modification and Removal")
    }
}
impl fmt::Debug for Track<InsertionAndDeletion> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Insertion and Deletion")
    }
}
impl fmt::Debug for Track<InsertionAndRemoval> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Insertion and Removal")
    }
}
impl fmt::Debug for Track<InsertionAndDeletionAndRemoval> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Insertion, Deletion and Removal")
    }
}
impl fmt::Debug for Track<Modification> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Modification")
    }
}
impl fmt::Debug for Track<ModificationAndDeletion> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Modification and Deletion")
    }
}
impl fmt::Debug for Track<ModificationAndRemoval> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Modification and Removal")
    }
}
impl fmt::Debug for Track<ModificationAndDeletionAndRemoval> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Modification, Deletion and Removal")
    }
}
impl fmt::Debug for Track<Deletion> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Deletion")
    }
}
impl fmt::Debug for Track<DeletionAndRemoval> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Deletion and Removal")
    }
}
impl fmt::Debug for Track<Removal> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Removal")
    }
}
impl fmt::Debug for Track<All> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("All")
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
    fn as_const() -> u32;

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

/// Returns `true` when the track timestamp is after the last time the system ran and before the current execution.
///
/// This method should only be necessary for custom storages that want to implement tracking.
#[inline]
pub fn is_track_within_bounds(timestamp: u32, last: u32, current: u32) -> bool {
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
