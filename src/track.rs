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
pub trait Tracking<T: Component>: Sized + Sealed {
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
    fn is_inserted(
        _sparse_set: &SparseSet<T, Self>,
        _entity: EntityId,
        _last: u32,
        _current: u32,
    ) -> bool {
        false
    }
    #[doc(hidden)]
    #[inline]
    fn is_modified(
        _sparse_set: &SparseSet<T, Self>,
        _entity: EntityId,
        _last: u32,
        _current: u32,
    ) -> bool {
        false
    }
    #[doc(hidden)]
    #[inline]
    fn is_deleted(
        _sparse_set: &SparseSet<T, Self>,
        _entity: EntityId,
        _last: u32,
        _current: u32,
    ) -> bool {
        false
    }
    #[doc(hidden)]
    #[inline]
    fn is_removed(
        _sparse_set: &SparseSet<T, Self>,
        _entity: EntityId,
        _last: u32,
        _current: u32,
    ) -> bool {
        false
    }

    #[doc(hidden)]
    fn remove(sparse_set: &mut SparseSet<T, Self>, entity: EntityId, current: u32) -> Option<T>;

    #[doc(hidden)]
    fn delete(sparse_set: &mut SparseSet<T, Self>, entity: EntityId, current: u32) -> bool;

    #[doc(hidden)]
    fn clear(sparse_set: &mut SparseSet<T, Self>, current: u32);

    #[doc(hidden)]
    fn apply<R, F: FnOnce(&mut T, &T) -> R>(
        sparse_set: &mut ViewMut<'_, T, Self>,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> R;

    #[doc(hidden)]
    fn apply_mut<R, F: FnOnce(&mut T, &mut T) -> R>(
        sparse_set: &mut ViewMut<'_, T, Self>,
        a: EntityId,
        b: EntityId,
        f: F,
    ) -> R;

    #[doc(hidden)]
    fn drain(sparse_set: &'_ mut SparseSet<T, Self>, current: u32) -> SparseSetDrain<'_, T>;

    #[doc(hidden)]
    fn clear_all_removed_or_deleted(_sparse_set: &mut SparseSet<T, Self>) {}
    #[doc(hidden)]
    fn clear_all_removed_or_deleted_older_than_timestamp(
        _sparse_set: &mut SparseSet<T, Self>,
        _timestamp: crate::TrackingTimestamp,
    ) {
    }
}

#[inline]
pub(crate) fn is_track_within_bounds(timestamp: u32, last: u32, current: u32) -> bool {
    let more_than_last = if timestamp < last {
        u32::MAX - last + timestamp
    } else {
        timestamp - last
    };
    let less_than_current = if current < timestamp {
        u32::MAX - timestamp + current
    } else {
        current - timestamp
    };

    more_than_last < u32::MAX / 2 && more_than_last > 0 && less_than_current < u32::MAX / 2
}
