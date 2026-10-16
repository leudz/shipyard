use crate::entity_id::EntityId;
use crate::iter::plan::SegmentSchedule;
use crate::iter::planner::NodeMetadata;
use crate::iter::EntityCursor;
use core::ptr::NonNull;

/// Entities driving a new iteration.
///
/// Holds a cursor over the candidates of the first segment and, when the traversal spans
/// multiple segments, the schedule of the follow-ups.\
/// An empty cursor never has follow-ups: constructors skip empty segments.
///
/// While a query is built, it also carries what its node knows about grouping.
/// Iterators drop it, it's never part of their state.
#[derive(Clone)]
#[doc(hidden)]
pub struct RawEntityIdAccess {
    pub(crate) cursor: EntityCursor,
    pub(crate) schedule: Option<SegmentSchedule>,
    pub(crate) metadata: NodeMetadata,
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
            metadata: NodeMetadata::Ungrouped,
        }
    }

    /// Returns an access without any entity.
    #[inline]
    pub(crate) fn empty() -> RawEntityIdAccess {
        RawEntityIdAccess {
            cursor: EntityCursor::empty(),
            schedule: None,
            metadata: NodeMetadata::Ungrouped,
        }
    }

    /// Replaces the grouping metadata.
    #[inline]
    pub(crate) fn with_metadata(mut self, metadata: NodeMetadata) -> RawEntityIdAccess {
        self.metadata = metadata;
        self
    }

    /// Marks these entities as coming from a filter, grouping is disabled for the whole query.
    #[inline]
    pub(crate) fn filtered(self) -> RawEntityIdAccess {
        self.with_metadata(NodeMetadata::Filtered)
    }

    /// Drops the grouping metadata, for state that outlives the construction.
    #[inline]
    pub(crate) fn without_metadata(self) -> RawEntityIdAccess {
        self.with_metadata(NodeMetadata::Ungrouped)
    }

    /// Returns the number of candidates, follow-ups included, `None` when it overflows.
    #[inline]
    pub(crate) fn candidate_count(&self) -> Option<usize> {
        let follow_ups_len = self
            .schedule
            .as_ref()
            .map_or(0, SegmentSchedule::remaining_len);

        self.cursor.len().checked_add(follow_ups_len)
    }
}
