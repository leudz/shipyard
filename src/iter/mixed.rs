use crate::entity_id::EntityId;
use crate::iter::plan::{Coverage, CoverageSpan, NodeKind, NodeState, PlanBuilder};
use crate::iter::{into_shiperator::strip_plus, Shiperator};
use crate::iter::{EntityCursor, RawEntityIdAccess, SegmentEntry, SegmentRow};
use core::ptr::NonNull;

const NON_CAPTAIN_FACTOR: f32 = 0.5;

/// [`Mixed::single_captain`] of a segment that needs the general resolver.
pub(crate) const NO_SINGLE_CAPTAIN: u32 = u32::MAX;

/// Iterator over multiple storages.
pub struct Mixed<S> {
    pub(crate) shiperator: S,
    /// Flags the immediate children acting as Captain in the current segment.\
    /// Nested `Mixed`s have their own mask.
    ///
    /// Captains can usually skip costly checks.
    pub(crate) captain_mask: u32,
    /// Position of the captain when the current segment has a single captain and no coverage,
    /// [`NO_SINGLE_CAPTAIN`] otherwise.
    pub(crate) single_captain: u32,
    /// Regions of our children's storages that earlier segments already walked.
    ///
    /// Candidates whose component falls in one of them are rejected.\
    /// Points into the plan of the last segment we entered, empty for ordinary traversals.
    pub(crate) coverage: NonNull<[Coverage]>,
}

// `coverage` only points to plain data that plans never modify and plans are `Sync`.
unsafe impl<S: Send> Send for Mixed<S> {}

impl<S: Clone> Clone for Mixed<S> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            shiperator: self.shiperator.clone(),
            captain_mask: self.captain_mask,
            single_captain: self.single_captain,
            coverage: self.coverage,
        }
    }
}

impl<S> Mixed<S> {
    /// Returns a `Mixed` driven by the children `captain_mask` flags, without coverage.
    #[inline]
    pub(crate) fn new(shiperator: S, captain_mask: u32) -> Mixed<S> {
        let mut mixed = Mixed {
            shiperator,
            captain_mask: 0,
            single_captain: NO_SINGLE_CAPTAIN,
            coverage: NonNull::from(&[][..]),
        };
        mixed.set_segment_state(captain_mask, &[]);

        mixed
    }

    /// Installs the captain mask and coverage of a segment, along with the resolver they select.
    ///
    /// `coverage` has to stay alive while we resolve captain indices for this segment.
    #[inline]
    fn set_segment_state(&mut self, captain_mask: u32, coverage: &[Coverage]) {
        self.captain_mask = captain_mask;
        self.coverage = NonNull::from(coverage);

        // Only the general resolver checks coverage.
        self.single_captain = if captain_mask.count_ones() == 1 && coverage.is_empty() {
            captain_mask.trailing_zeros()
        } else {
            NO_SINGLE_CAPTAIN
        };
    }

    /// Returns the coverage records of the current segment.
    ///
    /// ### Safety
    ///
    /// The plan of the last segment we entered, if any, must still be alive.
    #[inline]
    unsafe fn coverage_records(&self) -> &[Coverage] {
        // SAFE: `coverage` is either the empty slice `new` installed or records of the plan of
        // the last segment we entered, which our caller keeps alive. Plans never modify their records.
        unsafe { self.coverage.as_ref() }
    }
}

/// Embeds the follow-ups of `mixed`'s captain child in `mixed`'s own rows.
///
/// `entities` are the child's, its states start at index `slot` of `mixed`'s rows.\
/// Returns `entities` unchanged when the child has no follow-ups.
pub(crate) fn embed_captain_entities<S: Shiperator>(
    mixed: &S,
    slot: usize,
    entities: RawEntityIdAccess,
) -> RawEntityIdAccess {
    let Some(schedule) = &entities.schedule else {
        return entities;
    };

    // `mixed` is in its initial state, the captain child drives each follow-up in the same way.
    let row = SegmentRow::of(mixed);
    let mut plan = PlanBuilder::new(row.layout());
    plan.push_child_follow_ups(schedule, row.states(), slot);

    RawEntityIdAccess {
        cursor: entities.cursor,
        schedule: plan.finish(),
    }
}

macro_rules! impl_shiperator_output {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: Shiperator),+> Mixed<($($type,)+)> {
            /// Resolves the current candidate of `entities` with child `CAPTAIN` as the only captain.
            ///
            /// Roles are fixed at compile time, all other children are sailors.\
            /// It doesn't check coverage, `single_captain` only selects it for segments without any.
            ///
            /// ### Safety
            ///
            /// - `entities` must not be empty
            /// - `index` must be the local index of the current candidate of `entities`
            /// - the plan of the last segment we entered, if any, must still be alive
            #[inline(always)]
            unsafe fn single_captain_indices_of<const CAPTAIN: u32>(
                &self,
                entities: &EntityCursor,
                index: usize,
            ) -> Option<($($type::Index,)+)> {
                // SAFE: Our caller guarantees `entities` isn't empty.
                let eid = unsafe { entities.current() };

                // `$index == CAPTAIN` is a constant, each child keeps a single arm.
                Some(($(
                    if $index == CAPTAIN {
                        // SAFE: Our caller's guarantees are the captain's own.
                        unsafe { self.shiperator.$index.captain_indices_of(entities, index) }?
                    } else {
                        self.shiperator.$index.sailor_indices_of(eid)?
                    },
                )+))
            }

            /// Resolves the current candidate of `entities` for any captain mask then rejects it
            /// if an earlier segment already walked it.
            ///
            /// ### Safety
            ///
            /// - `entities` must not be empty
            /// - `index` must be the local index of the current candidate of `entities`
            /// - the plan of the last segment we entered, if any, must still be alive
            #[inline]
            unsafe fn checked_captain_indices_of(
                &self,
                entities: &EntityCursor,
                index: usize,
            ) -> Option<($($type::Index,)+)> {
                // SAFE: Our caller guarantees `entities` isn't empty.
                let eid = unsafe { entities.current() };

                // Captains can reject too, a tracking filter or a nested `Mixed` for example.
                let indices = ($(
                    if self.captain_mask & (1 << $index) != 0 {
                        // SAFE: Our caller's guarantees are the captain's own.
                        unsafe { self.shiperator.$index.captain_indices_of(entities, index) }?
                    } else {
                        self.shiperator.$index.sailor_indices_of(eid)?
                    },
                )+);

                // All indices are resolved but no component reference exists yet.
                // Rejecting here means a duplicate is never created, not even to be discarded.
                // SAFE: Our caller keeps the plan of our last segment alive.
                if unsafe { self.is_covered(&indices) } {
                    return None;
                }

                Some(indices)
            }

            /// Returns `true` when one of `indices` points into a region an earlier segment owns.
            ///
            /// ### Safety
            ///
            /// The plan of the last segment we entered, if any, must still be alive.
            #[inline]
            unsafe fn is_covered(&self, indices: &($($type::Index,)+)) -> bool {
                // SAFE: Our caller keeps the plan of our last segment alive.
                let coverage = unsafe { self.coverage_records() };
                if coverage.is_empty() {
                    return false;
                }

                // A record is about one child's storage, we match it against that child's
                // absolute index. Being present in that storage isn't enough, the component has
                // to be in the region the earlier segment walked.
                let storage_indices = [$($type::storage_index(&indices.$index)),+];

                coverage.iter().any(|record| {
                    let storage_index = storage_indices[record.child as usize]
                        .expect("coverage records name plain component windows");

                    (record.start..record.end).contains(&storage_index)
                })
            }
        }

        impl<$($type: Shiperator),+> Shiperator for Mixed<($($type,)+)> {
            type Out = ($($type::Out,)+);
            type Index = ($($type::Index,)+);

            #[inline]
            fn node_count() -> usize {
                1 $(+ $type::node_count())+
            }

            #[inline]
            fn describe(&self, active: bool, row: &mut SegmentRow) {
                let state = if active {
                    // Rows refer to coverage through spans of their plan, the records a segment
                    // installed can't be turned back into one. Composite constructors only
                    // describe nodes in their initial, ordinary state.
                    assert_eq!(self.coverage.len(), 0, "a Mixed with coverage can't be described");

                    NodeState::Mixed {
                        captain_mask: self.captain_mask,
                        coverage: CoverageSpan::EMPTY,
                    }
                } else {
                    NodeState::Sailor
                };
                row.push(NodeKind::Mixed { children: strip_plus!($(+{let _: $type; 1})+) }, state);

                $(
                    self.shiperator.$index.describe(active && self.captain_mask & (1 << $index) != 0, row);
                )+
            }

            #[inline(always)]
            unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
                let (captain_mask, coverage) = entry.next_mixed();

                // Captain children take their states, sailor children are skipped and left untouched.
                $(
                    if captain_mask & (1 << $index) != 0 {
                        // SAFE: Our caller guarantees `entry` holds our subtree's states for the segment
                        // about to be iterated. This child's states are next in preorder.
                        // The plan they come from is the one our caller keeps alive.
                        unsafe { self.shiperator.$index.enter_segment(entry) };
                    } else {
                        entry.skip($type::node_count());
                    }
                )+

                // `coverage` lives in the plan `entry` reads from,
                // our caller keeps it alive while we resolve captain indices.
                self.set_segment_state(captain_mask, coverage);
            }

            #[inline]
            #[allow(clippy::cast_precision_loss)]
            fn sail_time(&self) -> usize {
                strip_plus!($(+({
                    let sail_time = self.shiperator.$index.sail_time() as f32;

                    if self.captain_mask & (1 << $index) != 0 {
                        sail_time as usize
                    } else {
                        (sail_time * NON_CAPTAIN_FACTOR) as usize
                    }
                }))+)
            }

            #[inline]
            fn is_exact_sized(&self) -> bool {
                // Direct access isn't enough: every child has to drive the segment without ever
                // rejecting, and no candidate can be covered.
                self.coverage.len() == 0
                    $(&& self.captain_mask & (1 << $index) != 0 && self.shiperator.$index.is_exact_sized())+
            }

            #[inline(always)]
            unsafe fn captain_indices_of(&self, entities: &EntityCursor, index: usize) -> Option<Self::Index> {
                // SAFE: Our caller's guarantees are the resolvers' own.
                unsafe {
                    match self.single_captain {
                        $($index => self.single_captain_indices_of::<$index>(entities, index),)+
                        _ => self.checked_captain_indices_of(entities, index),
                    }
                }
            }

            // Membership ignores our captain mask and coverage. They describe the last segment we
            // drove, a parent asking about an entity doesn't care which segment walked it.
            #[inline]
            fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
                Some(($(
                    if let Some(index) = self.shiperator.$index.sailor_indices_of(eid) {
                        index
                    } else {
                        return None
                    },
                )+))
            }

            // A nested `Mixed` has its own coverage, it isn't a single storage.
            #[inline]
            fn storage_index(_index: &Self::Index) -> Option<usize> {
                None
            }

            #[inline]
            unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
                ($(
                    self.shiperator.$index.get_data(index.$index),
                )+)
            }
        }
    };
}

macro_rules! shiperator_output {
    ($(($type: ident, $index: tt))+; ($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_shiperator_output![$(($type, $index))*];
        shiperator_output![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;) => {
        impl_shiperator_output![$(($type, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
shiperator_output![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
#[cfg(feature = "extended_tuple")]
shiperator_output![
    (A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
    (K, 10) (L, 11) (M, 12) (N, 13) (O, 14) (P, 15) (Q, 16) (R, 17) (S, 18) (T, 19)
    (U, 20) (V, 21) (W, 22) (X, 23) (Y, 24) (Z, 25) (AA, 26) (BB, 27) (CC, 28) (DD, 29)
    (EE, 30) (FF, 31)
];
