use crate::entity_id::EntityId;
use crate::iter::plan::{NodeKind, NodeState, OrBranch};
use crate::iter::shiperator::fold_candidates;
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
                // SAFE: Our caller upholds `captain_indices_of`'s contract for the active branch.
                let Some(index) =
                    (unsafe { (self.storages).0.captain_indices_of(entities, index) })
                else {
                    return None;
                };

                Some(OneOfTwo::One(index))
            }
            OrBranch::B => {
                // SAFE: Our caller upholds `captain_indices_of`'s contract for the active branch.
                let Some(index) =
                    (unsafe { (self.storages).1.captain_indices_of(entities, index) })
                else {
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
        // SAFE: Each index was resolved by the storage of its branch, our caller upholds the rest.
        unsafe {
            match index {
                OneOfTwo::One(index) => OneOfTwo::One((self.storages).0.get_data(index)),
                OneOfTwo::Two(index) => OneOfTwo::Two((self.storages).1.get_data(index)),
            }
        }
    }

    #[inline]
    unsafe fn fold_segment<const WITH_ID: bool, Acc, Fold>(
        &self,
        entities: &mut EntityCursor,
        init: Acc,
        mut f: Fold,
    ) -> Acc
    where
        Fold: FnMut(Acc, Option<EntityId>, Self::Out) -> Acc,
    {
        match self.branch {
            // The first branch accepts exactly the candidates its own captain accepts,
            // so it can fold with its own loops. A nested `Mixed` keeps picking its loop
            // once per segment instead of resolving each candidate through us.
            // The adapter owns `f` with `move`, see `Shiper::fold_segments`.
            // SAFE: When the first branch drives, we entered it with our own segment,
            // `entities`' local indices are its local indices too.
            // Our caller keeps the plan of that segment alive.
            OrBranch::A => unsafe {
                (self.storages).0.fold_segment::<WITH_ID, Acc, _>(
                    entities,
                    init,
                    move |acc, entity_id, item| f(acc, entity_id, OneOfTwo::One(item)),
                )
            },
            // The second branch also rejects the entities the first one matches,
            // that check has to happen before any item is created.
            // SAFE: `fold_candidates` only resolves non-empty cursors at their current
            // candidate's local index, a local index of our segment per our caller.
            // Our caller keeps our plan alive. `captain_indices_of`'s results are valid for `get_data`.
            OrBranch::B => unsafe {
                fold_candidates::<Self, WITH_ID, Acc, Fold, _>(
                    self,
                    entities,
                    init,
                    f,
                    |entities, index| self.captain_indices_of(entities, index),
                )
            },
        }
    }
}
