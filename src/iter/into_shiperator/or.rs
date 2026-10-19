use crate::iter::plan::{OrBranch, PlanBuilder, SegmentRow};
use crate::iter::planner::NodeMetadata;
use crate::iter::{IntoShiperator, RawEntityIdAccess, Shiperator};
use crate::or::{Or, OrWindow};

// SAFE: Each branch's results come from its own implementation. Each branch's segments are
// embedded under rows built from our own description, with the branch driving them.
// An entity matching both branches is only yielded by the first one, the second branch looks
// candidates up in the first one and rejects them before creating any item.
unsafe impl<T: IntoShiperator, U: IntoShiperator> IntoShiperator for Or<(T, U)>
where
    T::Shiperator: Shiperator,
    U::Shiperator: Shiperator,
{
    type Shiperator = OrWindow<(T::Shiperator, U::Shiperator)>;

    #[inline]
    fn into_shiperator(self) -> (Self::Shiperator, usize, RawEntityIdAccess) {
        let (shiperator_a, _, entities_a) = (self.0).0.into_shiperator();
        let (shiperator_b, _, entities_b) = (self.0).1.into_shiperator();

        let mut or = OrWindow {
            storages: (shiperator_a, shiperator_b),
            branch: OrBranch::A,
        };

        // Rows of the whole Or while each branch drives it, with the branches' initial states.
        let row_a = SegmentRow::of(&or);
        or.branch = OrBranch::B;
        let row_b = SegmentRow::of(&or);

        // Preorder position of each branch's states in these rows.
        let slot_a = 1;
        let slot_b = 1 + T::Shiperator::node_count();

        // The first branch with candidates drives the initial segment, the Or is built with its
        // state installed. Its follow-ups, the other branch and that branch's own follow-ups
        // come after it. A branch without candidates doesn't have follow-ups and is left out.
        let mut initial = None;
        let mut plan = PlanBuilder::new(row_a.layout());
        let branches = [
            (OrBranch::A, entities_a, &row_a, slot_a),
            (OrBranch::B, entities_b, &row_b, slot_b),
        ];

        for (branch, entities, row, slot) in branches {
            if entities.cursor.is_empty() {
                debug_assert!(entities.schedule.is_none());
                continue;
            }

            if initial.is_none() {
                initial = Some((branch, entities.cursor));
            } else {
                plan.push_segment(entities.cursor, row.states());
            }

            if let Some(schedule) = &entities.schedule {
                plan.push_child_follow_ups(schedule, row.states(), slot);
            }
        }

        // `Or` keeps ordinary selection, with or without candidates.
        let Some((branch, cursor)) = initial else {
            or.branch = OrBranch::A;

            return (or, 0, RawEntityIdAccess::empty().filtered());
        };

        or.branch = branch;
        let entities = RawEntityIdAccess {
            cursor,
            schedule: plan.finish(),
            metadata: NodeMetadata::Filtered,
        };

        (or, cursor.len(), entities)
    }

    #[inline]
    fn can_captain() -> bool {
        true
    }

    #[inline]
    fn can_sailor() -> bool {
        true
    }
}
