mod all;
mod deletion;
mod insertion;
mod modification;
mod nothing;
mod removal;

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::seal::Sealed;
use crate::sparse_set::SparseSet;
use crate::view::ViewMut;
use crate::SparseSetDrain;

#[allow(missing_docs)]
pub struct Untracked(());
#[allow(missing_docs)]
pub struct Insertion(());
#[allow(missing_docs)]
pub struct Modification(());
#[allow(missing_docs)]
pub struct Deletion(());
#[allow(missing_docs)]
pub struct Removal(());
#[allow(missing_docs)]
pub struct All(());

/// Trait implemented by all trackings.
pub trait Tracking: 'static + Sized + Sealed + Send + Sync {
    #[doc(hidden)]
    #[inline]
    fn track_insertion() -> bool {
        false
    }
    #[doc(hidden)]
    #[inline]
    fn track_modification() -> bool {
        false
    }
    #[doc(hidden)]
    #[inline]
    fn track_deletion() -> bool {
        false
    }
    #[doc(hidden)]
    #[inline]
    fn track_removal() -> bool {
        false
    }

    #[doc(hidden)]
    #[inline]
    fn is_inserted<T: Component<Tracking = Self>>(
        _sparse_set: &SparseSet<T, Self>,
        _entity: EntityId,
        _last: u64,
        _current: u64,
    ) -> bool {
        false
    }
    #[doc(hidden)]
    #[inline]
    fn is_modified<T: Component<Tracking = Self>>(
        _sparse_set: &SparseSet<T, Self>,
        _entity: EntityId,
        _last: u64,
        _current: u64,
    ) -> bool {
        false
    }
    #[doc(hidden)]
    #[inline]
    fn is_deleted<T: Component<Tracking = Self>>(
        _sparse_set: &SparseSet<T, Self>,
        _entity: EntityId,
        _last: u64,
        _current: u64,
    ) -> bool {
        false
    }
    #[doc(hidden)]
    #[inline]
    fn is_removed<T: Component<Tracking = Self>>(
        _sparse_set: &SparseSet<T, Self>,
        _entity: EntityId,
        _last: u64,
        _current: u64,
    ) -> bool {
        false
    }

    #[doc(hidden)]
    fn remove<T: Component<Tracking = Self>>(
        sparse_set: &mut SparseSet<T, Self>,
        entity: EntityId,
        current: u64,
    ) -> Option<T>;

    #[doc(hidden)]
    fn delete<T: Component<Tracking = Self>>(
        sparse_set: &mut SparseSet<T, Self>,
        entity: EntityId,
        current: u64,
    ) -> bool;

    #[doc(hidden)]
    fn clear<T: Component<Tracking = Self>>(sparse_set: &mut SparseSet<T, Self>, current: u64);

    #[doc(hidden)]
    fn apply<T: Component<Tracking = Self>, R, F: FnOnce(&mut T, &T) -> R>(
        sparse_set: &mut ViewMut<'_, T, Self>,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> R;

    #[doc(hidden)]
    fn apply_mut<T: Component<Tracking = Self>, R, F: FnOnce(&mut T, &mut T) -> R>(
        sparse_set: &mut ViewMut<'_, T, Self>,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> R;

    #[doc(hidden)]
    fn drain<T: Component<Tracking = Self>>(
        sparse_set: &'_ mut SparseSet<T, Self>,
        current: u64,
    ) -> SparseSetDrain<'_, T>;

    #[doc(hidden)]
    fn clear_all_removed_or_deleted<T: Component<Tracking = Self>>(
        _sparse_set: &mut SparseSet<T, Self>,
    ) {
    }
    #[doc(hidden)]
    fn clear_all_removed_or_deleted_older_than_timestamp<T: Component<Tracking = Self>>(
        _sparse_set: &mut SparseSet<T, Self>,
        _timestamp: crate::TrackingTimestamp,
    ) {
    }
}

#[inline]
pub(crate) fn is_track_within_bounds(timestamp: u64, last: u64, current: u64) -> bool {
    let more_than_last = if timestamp < last {
        u64::MAX - last + timestamp
    } else {
        timestamp - last
    };
    let less_than_current = if current < timestamp {
        u64::MAX - timestamp + current
    } else {
        current - timestamp
    };

    more_than_last < u64::MAX / 2 && more_than_last > 0 && less_than_current < u64::MAX / 2
}
