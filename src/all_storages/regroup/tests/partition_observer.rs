use super::{Builder, Placement};
use crate::storage::StorageId;
use alloc::vec::Vec;

/// Unit tests observe which partition path ran and the eligibility conditions at that point.
#[derive(Debug)]
pub(in super::super) struct PartitionCheck {
    pub(in super::super) storage_id: StorageId,
    pub(in super::super) rows_covered: bool,
    pub(in super::super) untouched: bool,
    pub(in super::super) unpromoted: bool,
    pub(in super::super) no_departures: bool,
    pub(in super::super) used_exceptions: bool,
}

std::thread_local! {
    pub(in super::super) static PARTITION_CHECKS: core::cell::RefCell<Option<Vec<PartitionCheck>>> =
        const { core::cell::RefCell::new(None) };
}

impl Builder<'_, '_> {
    /// Records the partition decision only while a guard test is observing it.
    pub(super) fn observe_partition(
        &self,
        index: usize,
        placements: &[Placement],
        used_exceptions: bool,
    ) {
        PARTITION_CHECKS.with_borrow_mut(|checks| {
            let Some(checks) = checks else {
                return;
            };
            let membership = self.memberships.storages[index];
            checks.push(PartitionCheck {
                storage_id: membership.storage_id,
                rows_covered: membership.groups.unevaluated_start == self.dense[index].len()
                    || self.memberships.alone[index],
                untouched: self.touched[index].is_none(),
                unpromoted: self.plan.storage(&membership.storage_id).promoted == 0,
                no_departures: placements
                    .iter()
                    .all(|placement| self.plan.groups[placement.group_index].leaving == 0),
                used_exceptions,
            });
        });
    }
}
