use crate::entity_id::EntityId;
use crate::iter::plan::{NodeKind, NodeState, OrBranch};
use crate::iter::{EntityCursor, SegmentEntry, SegmentRow, Shiperator};
use crate::or::{OneOfTwo, OrWindow};

impl<T: Shiperator, U: Shiperator> Shiperator for OrWindow<(T, U)> {
    type Out = OneOfTwo<T::Out, U::Out>;
    type Index = OneOfTwo<T::Index, U::Index>;

    #[inline]
    fn node_count() -> usize {
        1 + T::node_count() + U::node_count()
    }

    #[inline]
    fn describe(&self, active: bool, row: &mut SegmentRow) {
        let state = if active {
            NodeState::Or {
                branch: self.branch,
            }
        } else {
            NodeState::Sailor
        };
        row.push(NodeKind::Or, state);

        (self.storages)
            .0
            .describe(active && self.branch == OrBranch::A, row);
        (self.storages)
            .1
            .describe(active && self.branch == OrBranch::B, row);
    }

    #[inline]
    unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
        self.branch = entry.next_or();

        // Only the driving branch takes its states, the other one stays a sailor.
        // SAFE: Our caller guarantees `entry` holds our subtree's states for the segment about
        // to be iterated. The driving branch's states follow ours in preorder,
        // after the first branch's when it's the second one.
        match self.branch {
            OrBranch::A => {
                unsafe { (self.storages).0.enter_segment(entry) };
                entry.skip(U::node_count());
            }
            OrBranch::B => {
                entry.skip(T::node_count());
                unsafe { (self.storages).1.enter_segment(entry) };
            }
        }
    }

    #[inline]
    fn sail_time(&self) -> usize {
        (self.storages).0.sail_time() + (self.storages).1.sail_time()
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        false
    }

    #[inline]
    unsafe fn captain_indices_of(
        &self,
        entities: &EntityCursor,
        index: usize,
    ) -> Option<Self::Index> {
        match self.branch {
            OrBranch::A => {
                let Some(index) = (self.storages).0.captain_indices_of(entities, index) else {
                    return None;
                };

                Some(OneOfTwo::One(index))
            }
            OrBranch::B => {
                let Some(index) = (self.storages).1.captain_indices_of(entities, index) else {
                    return None;
                };

                // The first branch already yielded the entities it matches.
                // SAFE: Our caller guarantees `entities` isn't empty.
                let eid = unsafe { entities.current() };

                if (self.storages).0.sailor_indices_of(eid).is_some() {
                    return None;
                }

                Some(OneOfTwo::Two(index))
            }
        }
    }

    #[inline]
    fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
        // Option::map can throw the compiler off
        // I prefer taking the readability hit
        #[allow(clippy::manual_map)]
        if let Some(index) = (self.storages).0.sailor_indices_of(eid) {
            Some(OneOfTwo::One(index))
        } else if let Some(index) = (self.storages).1.sailor_indices_of(eid) {
            Some(OneOfTwo::Two(index))
        } else {
            None
        }
    }

    #[inline]
    fn storage_index(_index: &Self::Index) -> Option<usize> {
        None
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        match index {
            OneOfTwo::One(index) => OneOfTwo::One((self.storages).0.get_data(index)),
            OneOfTwo::Two(index) => OneOfTwo::Two((self.storages).1.get_data(index)),
        }
    }
}
