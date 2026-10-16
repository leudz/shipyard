use crate::entity_id::EntityId;
use crate::iter::plan::SegmentSchedule;
use crate::iter::EntityCursor;
use core::ptr::NonNull;

/// Entities driving a new iteration.
///
/// Holds a cursor over the candidates of the first segment and, when the traversal spans
/// multiple segments, the schedule of the follow-ups.\
/// An empty cursor never has follow-ups: constructors skip empty segments.
#[derive(Clone)]
#[doc(hidden)]
pub struct RawEntityIdAccess {
    pub(crate) cursor: EntityCursor,
    pub(crate) schedule: Option<SegmentSchedule>,
}

impl RawEntityIdAccess {
    /// Returns an access over the `len` entities starting at `start`, without follow-ups.
    ///
    /// ### Safety
    ///
    /// When `len` isn't 0:
    /// - `start` must point to `len` consecutive initialized entities within a single allocation
    /// - these entities must not be deallocated or modified while this access, any clone of it
    ///   or any iterator built from them is used
    #[inline]
    pub(crate) unsafe fn new(start: NonNull<EntityId>, len: usize) -> RawEntityIdAccess {
        RawEntityIdAccess {
            // SAFE: Our caller upholds `EntityCursor::new`'s requirements, they are the same as ours.
            cursor: unsafe { EntityCursor::new(start, len) },
            schedule: None,
        }
    }

    /// Returns an access without any entity.
    #[inline]
    pub(crate) fn empty() -> RawEntityIdAccess {
        RawEntityIdAccess {
            cursor: EntityCursor::empty(),
            schedule: None,
        }
    }
}
