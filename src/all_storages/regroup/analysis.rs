//! Finds what regroup has to change, before any id moves.
//!
//! Every member of a group agrees on the logical positions below all their trusts, the group's
//! kept prefix. Its entities stay in place unless they have to leave, and the entities of an
//! ungrouped region stay there unless they have to join a group. Regroup only looks at the rest:
//! the untrusted suffixes and the unevaluated regions. From these alone it counts, for each group,
//! the entities leaving its kept prefix and the ones joining it, and for each storage,
//! the ungrouped components joining a group. Scanning follows the untrusted suffixes and
//! unevaluated regions, not the number of changes: even a few removals can leave a large
//! suffix untrusted. Clean ranges and evaluated ungrouped regions aren't scanned.
//!
//! Sparse arrays and `Groups` are left as regroup found them, frozen rows are read against them.
//! The only change is attaching the unions that don't exist yet, as empty ranges holding no row.

use super::{Selected, original_row};
use crate::entity_id::EntityId;
use crate::sparse_set::{Groups, Ring, SparseArray};
use crate::storage::StorageId;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// What classifying an entity reads of a grouped storage.
#[derive(Clone, Copy)]
pub(super) struct Membership<'a> {
    pub(super) storage_id: StorageId,
    pub(super) groups: &'a Groups,
    pub(super) sparse: &'a SparseArray,
}

/// The membership of every grouped storage, sorted by storage id.
pub(super) struct Memberships<'a> {
    pub(super) storages: Vec<Membership<'a>>,
    /// For each storage, the positions in `storages` of the members of each of its ranges,
    /// one range after the other.
    ///
    /// Discovery runs for every entity it looks at, finding storages by id each time would cost
    /// more than its sparse lookups.
    members: Vec<Vec<usize>>,
    /// For each storage, `true` when it holds a single range whose members hold no other range.
    alone: Vec<bool>,
}

impl<'a> Memberships<'a> {
    /// Gathers the membership of every grouped storage, `storages` has to be sorted by storage id.
    pub(super) fn new(storages: Vec<Membership<'a>>) -> Memberships<'a> {
        let mut memberships = Memberships {
            storages,
            members: Vec::new(),
            alone: Vec::new(),
        };

        memberships.members = memberships
            .storages
            .iter()
            .map(|membership| {
                membership
                    .groups
                    .ranges
                    .iter()
                    .flat_map(|range| range.group.iter())
                    .map(|member_id| memberships.position(member_id))
                    .collect()
            })
            .collect();

        memberships.alone = (0..memberships.storages.len())
            .map(|index| {
                memberships.storages[index].groups.ranges.len() == 1
                    && memberships.members[index]
                        .iter()
                        .all(|&member| memberships.storages[member].groups.ranges.len() == 1)
            })
            .collect();

        memberships
    }

    /// Returns the position in `storages` of the storage with id `storage_id`.
    ///
    /// ### Panics
    ///
    /// - The storage has no range.
    pub(super) fn position(&self, storage_id: &StorageId) -> usize {
        self.storages
            .binary_search_by_key(storage_id, |membership| membership.storage_id)
            .expect("group members are grouped storages")
    }

    /// Returns the membership of the storage with id `storage_id`.
    ///
    /// ### Panics
    ///
    /// - The storage has no range.
    pub(super) fn get(&self, storage_id: &StorageId) -> &Membership<'a> {
        &self.storages[self.position(storage_id)]
    }
}

/// Shared references to every selected grouped storage, as regroup found them.
pub(super) struct Frozen<'a> {
    pub(super) memberships: Memberships<'a>,
    /// The ids of each storage of `memberships`, at the same position.
    pub(super) dense: Vec<&'a [EntityId]>,
}

impl<'a> Frozen<'a> {
    /// Borrows every selected storage that has at least one range.
    pub(super) fn new(selected: &'a Selected<'_>) -> Frozen<'a> {
        let mut memberships = Vec::new();
        let mut dense = Vec::new();

        for storage_id in selected.ids() {
            let storage = selected.get(storage_id);
            let Some(groups) = storage.groups().filter(|groups| groups.has_ranges()) else {
                continue;
            };

            memberships.push(Membership {
                storage_id: *storage_id,
                groups,
                sparse: storage.sparse_array().expect("group members are SparseSet"),
            });
            dense.push(storage.dense().expect("group members are SparseSet"));
        }

        Frozen {
            memberships: Memberships::new(memberships),
            dense,
        }
    }
}

/// A step of a storage's classification tree.
#[derive(Clone, Copy)]
enum Node {
    /// No entity got here yet.
    Unexplored,
    /// Checks whether the entity owns a component in the storage at `position`,
    /// `next[present]` is the node after it.
    Probe { position: usize, next: [usize; 2] },
    /// Where the classification ends: the position of the group's plan,
    /// `None` for the ungrouped region.
    Leaf(Option<usize>),
}

/// What classifications from one storage found, as a tree of lookups whose root is node 0.
struct Tree {
    nodes: Vec<Node>,
    /// The probe leading to each node, at the same position. The root's is itself.
    ///
    /// Only learning a new path reads it, walks don't.
    parents: Vec<usize>,
}

impl Tree {
    fn new() -> Tree {
        Tree {
            nodes: alloc::vec![Node::Unexplored],
            parents: alloc::vec![0],
        }
    }
}

/// Scratch buffers finding the storages an entity has to be grouped in, reused across entities.
///
/// It also remembers what each discovery found, see [`classify`](Self::classify). What it
/// remembers only holds for the `Memberships` it was found with, [`forget`](Self::forget) drops it.
#[derive(Default)]
pub(super) struct Discovery {
    /// Positions in `Memberships::storages` of the members of the groups found so far,
    /// sorted once discovery is complete.
    members: Vec<usize>,
    /// Groups whose satisfaction we already checked, by address.
    checked: Vec<*const StorageId>,
    /// The ids of the sorted `members`.
    ids: Vec<StorageId>,
    /// Answers discovery uses without a lookup, as storage position and presence.
    known: Vec<(usize, bool)>,
    /// The storage position and presence of every answer the last discovery used, in order,
    /// known ones included.
    probes: Vec<(usize, bool)>,
    /// One classification tree per storage position. Empty until the first classification.
    trees: Vec<Tree>,
    /// Unit tests use this to check that learning doesn't repeat the walk's sparse lookups.
    #[cfg(test)]
    pub(super) checks: usize,
}

impl Discovery {
    /// Returns the position of the plan of the group `entity`'s component in the storage at
    /// `index` of `memberships` goes to, `None` when it goes to the ungrouped region.
    ///
    /// `entity` has to own a component in that storage, generation included.
    ///
    /// It's [`traverse`](Self::traverse) with `resolve` turning the sorted member ids it finds
    /// into a plan position, but it remembers the answers. `resolve` is only called for entities
    /// that answer differently from every one classified from the same storage before.
    /// It has to give the same position for the same members as long as we remember.
    pub(super) fn classify(
        &mut self,
        memberships: &Memberships<'_>,
        index: usize,
        entity: EntityId,
        mut resolve: impl FnMut(&[StorageId]) -> usize,
    ) -> Option<usize> {
        self.walk(memberships, index, entity, &mut resolve)
    }

    /// Classifies `entity` by walking the tree of the storage at `index`,
    /// growing it with a discovery when it gets somewhere unexplored.
    ///
    /// The lookups discovery makes only depend on where it starts and the answers it got so far,
    /// so entities answering the same go down the same path. Each node checks a storage the path
    /// didn't check yet, so a walk never makes more lookups than discovery, often fewer.
    fn walk(
        &mut self,
        memberships: &Memberships<'_>,
        index: usize,
        entity: EntityId,
        resolve: &mut impl FnMut(&[StorageId]) -> usize,
    ) -> Option<usize> {
        if self.trees.is_empty() {
            self.trees = (0..memberships.storages.len())
                .map(|_| Tree::new())
                .collect();
        }

        let nodes = &self.trees[index].nodes;
        let mut node = 0;
        loop {
            match nodes[node] {
                Node::Probe { position, next } => {
                    #[cfg(test)]
                    {
                        self.checks += 1;
                    }
                    let present = memberships.storages[position].sparse.contains(entity);

                    node = next[usize::from(present)];
                }
                Node::Leaf(group_index) => return group_index,
                Node::Unexplored => break,
            }
        }

        self.learn(memberships, index, entity, node, resolve)
    }

    /// Classifies `entity` with a discovery, then grows the tree of the storage at `index` with
    /// its path from `node`, the unexplored node the walk stopped at.
    fn learn(
        &mut self,
        memberships: &Memberships<'_>,
        index: usize,
        entity: EntityId,
        node: usize,
        resolve: &mut impl FnMut(&[StorageId]) -> usize,
    ) -> Option<usize> {
        // We read the walk's answers back from the tree, discovery doesn't look them up again.
        let tree = &self.trees[index];
        self.known.clear();
        let mut child = node;
        while child != 0 {
            let parent = tree.parents[child];
            let Node::Probe { position, next } = tree.nodes[parent] else {
                unreachable!("only probes lead to other nodes");
            };

            self.known.push((position, child == next[1]));
            child = parent;
        }
        let walked = self.known.len();

        let group_index = self.traverse(memberships, index, entity).map(resolve);

        // A path checks each storage once, the answer can't change within a discovery.
        // Discovery starts with the walk's answers, they're its first `walked` ones.
        let probes = &self.probes;
        let first_probes = probes
            .iter()
            .enumerate()
            .filter(|&(i, &(position, _))| probes[..i].iter().all(|&(seen, _)| seen != position))
            .map(|(_, &probe)| probe)
            .skip(walked);

        let tree = &mut self.trees[index];
        let mut node = node;
        for (position, present) in first_probes {
            let absent = tree.nodes.len();
            tree.nodes.extend([Node::Unexplored, Node::Unexplored]);
            tree.parents.extend([node, node]);
            tree.nodes[node] = Node::Probe {
                position,
                next: [absent, absent + 1],
            };
            node = absent + usize::from(present);
        }
        tree.nodes[node] = Node::Leaf(group_index);

        group_index
    }

    /// Drops what classifications found, for when the `Memberships` they were found with change.
    pub(super) fn forget(&mut self) {
        self.trees.clear();
    }

    /// Returns the sorted ids of the storages `entity` has to be grouped in, starting from its
    /// component in the storage at `index` of `memberships`.
    ///
    /// `entity` has to own a component in that storage, generation included.
    ///
    /// These are the members of every group `entity` satisfies that's connected to that storage
    /// through other groups it satisfies. Groups it doesn't satisfy don't connect anything,
    /// and owning the components of two disjoint groups doesn't merge them.\
    /// Returns `None` when `entity` satisfies none of that storage's groups.
    ///
    /// It takes the answers in `known` as they are, without a lookup. Every answer it uses ends up
    /// in `probes`, in order.
    fn traverse(
        &mut self,
        memberships: &Memberships<'_>,
        index: usize,
        entity: EntityId,
    ) -> Option<&[StorageId]> {
        self.probes.clear();

        // Groups only connect through shared members, nothing connects to a group whose members
        // are in no other group. Checking it is all there is to do, and it's the common case.
        if memberships.alone[index] {
            let positions = &memberships.members[index];
            let satisfied = positions.iter().all(|&position| {
                position == index || {
                    let present = match self.known.iter().find(|&&(known, _)| known == position) {
                        Some(&(_, present)) => present,
                        None => {
                            #[cfg(test)]
                            {
                                self.checks += 1;
                            }
                            memberships.storages[position].sparse.contains(entity)
                        }
                    };
                    self.probes.push((position, present));

                    present
                }
            });
            if !satisfied {
                return None;
            }

            // A range's members are sorted by id, so are their positions.
            self.ids.clear();
            self.ids.extend(
                positions
                    .iter()
                    .map(|&position| memberships.storages[position].storage_id),
            );

            return Some(&self.ids);
        }

        self.members.clear();
        self.checked.clear();
        self.members.push(index);

        // `members` grows while we go through it, each member's groups are inspected once.
        // A group we check from one member is skipped from the others, it has the same answer.
        let mut next = 0;
        while let Some(&member) = self.members.get(next) {
            next += 1;

            // The positions of each range's members follow each other.
            let mut positions = memberships.members[member].as_slice();
            for range in &memberships.storages[member].groups.ranges {
                let (group_positions, rest) = positions.split_at(range.group.len());
                positions = rest;

                let group = Arc::as_ptr(&range.group).cast::<StorageId>();
                if self.checked.contains(&group) {
                    continue;
                }
                self.checked.push(group);

                // Every storage in `members` holds a component of `entity`,
                // we only look at the others.
                let satisfied = group_positions.iter().all(|position| {
                    self.members.contains(position) || {
                        let present = match self.known.iter().find(|&(known, _)| known == position)
                        {
                            Some(&(_, present)) => present,
                            None => {
                                #[cfg(test)]
                                {
                                    self.checks += 1;
                                }
                                memberships.storages[*position].sparse.contains(entity)
                            }
                        };
                        self.probes.push((*position, present));

                        present
                    }
                });
                if !satisfied {
                    continue;
                }

                for position in group_positions {
                    if !self.members.contains(position) {
                        self.members.push(*position);
                    }
                }
            }
        }

        // A satisfied group would have added at least one other storage.
        if self.members.len() == 1 {
            return None;
        }

        // Storages are sorted by id, sorting their positions sorts their ids.
        self.members.sort_unstable();
        self.ids.clear();
        self.ids.extend(
            self.members
                .iter()
                .map(|&position| memberships.storages[position].storage_id),
        );

        Some(&self.ids)
    }
}

/// A range's ids in logical order, as the two pieces its wrap point splits them in.
///
/// Finding a logical position in a ring divides, this does it once per range instead of per row.
#[derive(Clone, Copy)]
struct Logical<'a> {
    /// Logical positions `0..head.len()`, from the wrap point to the end of the range.
    head: &'a [EntityId],
    /// The logical positions after them, from the start of the range to the wrap point.
    tail: &'a [EntityId],
}

impl<'a> Logical<'a> {
    fn new(dense: &'a [EntityId], ring: Ring) -> Logical<'a> {
        if ring.len == 0 {
            return Logical {
                head: &[],
                tail: &[],
            };
        }

        let wrap_point = ring.start + ring.wrap_offset();

        Logical {
            head: &dense[wrap_point..ring.end()],
            tail: &dense[ring.start..wrap_point],
        }
    }

    /// Returns the id at `logical`, `None` past the range's end.
    fn get(&self, logical: usize) -> Option<EntityId> {
        match self.head.get(logical) {
            Some(&entity) => Some(entity),
            None => self.tail.get(logical - self.head.len()).copied(),
        }
    }
}

/// What regroup decided for a group, the same in each of its members.
///
/// The range ends up holding `kept - leaving + incoming` entities.
pub(super) struct GroupPlan {
    /// The group's sorted member ids, compared by pointer.
    pub(super) group: Arc<[StorageId]>,
    /// Length of the logical prefix every member trusts, the entities in it stay unless they leave.
    pub(super) kept: usize,
    /// Number of entities of the kept prefix that have to leave the group.
    pub(super) leaving: usize,
    /// Number of entities joining the range after what stays of the kept prefix.
    pub(super) incoming: usize,
}

/// What regroup decided for a grouped storage.
pub(super) struct StoragePlan {
    pub(super) storage_id: StorageId,
    /// Number of components of the ungrouped region that have to join a group.
    pub(super) promoted: usize,
}

/// What regroup has to change.
pub(super) struct Plan {
    /// Every group, existing ones first. New unions come after them, in the order we found them.
    pub(super) groups: Vec<GroupPlan>,
    /// Every grouped storage, sorted by id.
    pub(super) storages: Vec<StoragePlan>,
    /// Number of `groups` attached to their members, the ones after it are new unions to attach.
    attached: usize,
}

impl Plan {
    /// Starts a plan where every group keeps its trusted prefix and nothing moves.
    fn new(memberships: &Memberships<'_>) -> Plan {
        let mut groups: Vec<GroupPlan> = Vec::new();
        for membership in &memberships.storages {
            for range in &membership.groups.ranges {
                if groups
                    .iter()
                    .any(|plan| Arc::ptr_eq(&plan.group, &range.group))
                {
                    continue;
                }

                // Members agree on every position below all their trusts.
                let kept = range
                    .group
                    .iter()
                    .map(|member_id| {
                        memberships
                            .get(member_id)
                            .groups
                            .range(&range.group)
                            .expect("every member holds a range for its group")
                            .trust
                    })
                    .min()
                    .expect("a group has at least two members");

                groups.push(GroupPlan {
                    group: range.group.clone(),
                    kept,
                    leaving: 0,
                    incoming: 0,
                });
            }
        }

        let storages = memberships
            .storages
            .iter()
            .map(|membership| StoragePlan {
                storage_id: membership.storage_id,
                promoted: 0,
            })
            .collect();

        Plan {
            attached: groups.len(),
            groups,
            storages,
        }
    }

    /// Returns the position of `group`'s plan.
    ///
    /// Groups are compared by pointer.
    pub(super) fn group_index(&self, group: &Arc<[StorageId]>) -> usize {
        self.groups
            .iter()
            .position(|plan| Arc::ptr_eq(&plan.group, group))
            .expect("every attached group has a plan")
    }

    /// Returns the plan of the storage with id `storage_id`.
    pub(super) fn storage(&self, storage_id: &StorageId) -> &StoragePlan {
        &self.storages[self.storage_index(storage_id)]
    }

    fn storage_mut(&mut self, storage_id: &StorageId) -> &mut StoragePlan {
        let index = self.storage_index(storage_id);

        &mut self.storages[index]
    }

    fn storage_index(&self, storage_id: &StorageId) -> usize {
        self.storages
            .binary_search_by_key(storage_id, |plan| plan.storage_id)
            .expect("every grouped storage has a plan")
    }

    /// Returns the position of the plan of the group made of the sorted `members`, if any.
    ///
    /// This compares by value, so each set of members has a single identity,
    /// whether it was declared or created as a union before.
    pub(super) fn find(&self, members: &[StorageId]) -> Option<usize> {
        self.groups.iter().position(|plan| *plan.group == *members)
    }

    /// Returns the position of the plan of the group made of the sorted `members`,
    /// adding a new union when there's none.
    fn find_or_add(&mut self, members: &[StorageId]) -> usize {
        if let Some(index) = self.find(members) {
            return index;
        }

        self.groups.push(GroupPlan {
            group: Arc::from(members),
            kept: 0,
            leaving: 0,
            incoming: 0,
        });

        self.groups.len() - 1
    }

    /// Finds where `entity`'s component in the storage at `index` goes, and counts it.
    ///
    /// Returns the position of the plan of its group, `None` when it goes to the ungrouped region.
    fn count(
        &mut self,
        memberships: &Memberships<'_>,
        discovery: &mut Discovery,
        index: usize,
        entity: EntityId,
    ) -> Option<usize> {
        let storage_id = memberships.storages[index].storage_id;
        let group_index = discovery.classify(memberships, index, entity, |members| {
            self.find_or_add(members)
        })?;

        // Every member holds a component of the entity, we count it from the first one only.
        if self.groups[group_index].group[0] == storage_id {
            self.groups[group_index].incoming += 1;
        }

        Some(group_index)
    }

    /// Counts where every unevaluated component goes, and the components their entity moves
    /// out of kept prefixes and ungrouped regions.
    fn scan_unevaluated(&mut self, frozen: &Frozen<'_>, discovery: &mut Discovery) {
        let storages = frozen.memberships.storages.iter().zip(&frozen.dense);
        for (index, (membership, dense)) in storages.enumerate() {
            for &entity in &dense[membership.groups.unevaluated_start..] {
                let storage_id = &membership.storage_id;
                let Some(group_index) = self.count(&frozen.memberships, discovery, index, entity)
                else {
                    continue;
                };

                if self.is_first_unevaluated(&frozen.memberships, group_index, storage_id, entity) {
                    self.count_departures(&frozen.memberships, group_index, storage_id, entity);
                }
            }
        }
    }

    /// Returns `true` when `storage_id` is the first member of the group of `group_index`
    /// holding an unevaluated component of `entity`.
    ///
    /// An entity can gain several components, only one of them counts its departures.
    pub(super) fn is_first_unevaluated(
        &self,
        memberships: &Memberships<'_>,
        group_index: usize,
        storage_id: &StorageId,
        entity: EntityId,
    ) -> bool {
        self.groups[group_index]
            .group
            .iter()
            .take_while(|&member_id| member_id < storage_id)
            .all(|member_id| {
                let member = memberships.get(member_id);
                let row = original_row(member.sparse, entity)
                    .expect("the entity satisfies the group, it owns a component in each member");

                row < member.groups.unevaluated_start
            })
    }

    /// Counts the components of `entity` that leave a kept prefix or an ungrouped region
    /// to join the group of `group_index`, `storage_id` holding an unevaluated one.
    ///
    /// They're the components sitting where nothing else is scanned.
    /// Unevaluated components and the ones past a kept prefix are counted where they are.
    fn count_departures(
        &mut self,
        memberships: &Memberships<'_>,
        group_index: usize,
        storage_id: &StorageId,
        entity: EntityId,
    ) {
        // We read the members by position, `self` changes in the loop.
        for position in 0..self.groups[group_index].group.len() {
            let member_id = self.groups[group_index].group[position];
            if &member_id == storage_id {
                continue;
            }

            let member = memberships.get(&member_id);
            let row = original_row(member.sparse, entity)
                .expect("the entity satisfies the group, it owns a component in each member");

            let departs = if let Some(range_index) = member.groups.range_index_of(row) {
                let range = &member.groups.ranges[range_index];
                let source_index = self.group_index(&range.group);
                let source = &mut self.groups[source_index];
                let is_kept = range.ring().logical_of(row) < source.kept;

                // Kept entities are aligned in every member, the one in `storage_id` would be
                // there too instead of being unevaluated.
                debug_assert!(source_index != group_index || !is_kept);

                // The entity sits in the kept prefix of every member of the source.
                // It owns all of them, so they're all members of the destination,
                // and we count it once, in the first one.
                if is_kept && member_id == source.group[0] {
                    source.leaving += 1;
                }

                is_kept
            } else if row < member.groups.unevaluated_start {
                self.storage_mut(&member_id).promoted += 1;

                true
            } else {
                false
            };

            if departs && position == 0 {
                self.groups[group_index].incoming += 1;
            }
        }
    }

    /// Counts where every component past its range's kept prefix goes.
    ///
    /// A row whose entity sits in the range of the same group in every other member doesn't
    /// need a classification when we know that group is still its destination, see below.
    fn scan_evaluated(&mut self, frozen: &Frozen<'_>, discovery: &mut Discovery) {
        // Every component added since the last regroup is unevaluated. With none, no entity
        // gained a component since, and the ones in a range had that range's group as their
        // destination. Owning fewer components can't make a group they still own all the
        // members of anything but their destination: what they satisfy only shrinks.
        // Rows in a range only move within it or leave it, so a row found in a range was there.
        let all_evaluated = frozen
            .memberships
            .storages
            .iter()
            .zip(&frozen.dense)
            .all(|(membership, dense)| membership.groups.unevaluated_start == dense.len());

        // The other members' ranges, in logical order, reused across ranges.
        let mut others: Vec<Logical<'_>> = Vec::new();

        let storages = frozen.memberships.storages.iter().zip(&frozen.dense);
        for (index, (membership, dense)) in storages.enumerate() {
            // The positions of each range's members follow each other.
            let mut positions = frozen.memberships.members[index].as_slice();
            for range in &membership.groups.ranges {
                let (group_positions, rest) = positions.split_at(range.group.len());
                positions = rest;

                let group_index = self.group_index(&range.group);
                let kept = self.groups[group_index].kept;
                let ring = range.ring();

                // For a storage whose only group connects to nothing, owning every member
                // is the whole answer, unevaluated rows or not.
                let can_match = all_evaluated || frozen.memberships.alone[index];

                others.clear();
                if can_match {
                    for &position in group_positions {
                        if position == index {
                            continue;
                        }

                        let other = frozen.memberships.storages[position]
                            .groups
                            .range(&range.group)
                            .expect("every member holds a range for its group");
                        others.push(Logical::new(frozen.dense[position], other.ring()));
                    }
                }

                let is_first = range.group[0] == membership.storage_id;
                for logical in kept..range.len {
                    let entity = dense[ring.physical_of(logical)];

                    // Equal ids prove the entity owns every member, whatever their positions.
                    // Pairing by logical position only decides how often we find them.
                    if can_match
                        && others
                            .iter()
                            .all(|other| other.get(logical) == Some(entity))
                    {
                        if is_first {
                            self.groups[group_index].incoming += 1;
                        }

                        continue;
                    }

                    self.count(&frozen.memberships, discovery, index, entity);
                }
            }
        }
    }

    /// Attaches the new unions to each of their members, as empty ranges after their last range.
    ///
    /// A union only joins groups that share members, so its members are selected too.
    fn attach_new(&mut self, selected: &mut Selected<'_>) {
        for plan in &self.groups[self.attached..] {
            for member_id in plan.group.iter() {
                selected
                    .get_mut(member_id)
                    .groups_mut()
                    .expect("group members are SparseSet")
                    .attach(plan.group.clone(), *member_id);
            }
        }

        self.attached = self.groups.len();
    }
}

/// Decides what regroup has to change in the `selected` storages, `None` when nothing has to.
///
/// Attaches the unions entities need that don't exist yet, as empty ranges. Nothing else changes.
pub(super) fn analyze(selected: &mut Selected<'_>) -> Option<Plan> {
    if is_clean(selected) {
        return None;
    }

    let mut discovery = Discovery::default();

    // Unevaluated components are the only ones that can make an entity leave a kept prefix or
    // an ungrouped region: it has to gain a component to join a larger group.
    // Losing one lowers that member's trust, so the entity is past the kept prefix.
    let frozen = Frozen::new(selected);
    let mut plan = Plan::new(&frozen.memberships);
    plan.scan_unevaluated(&frozen, &mut discovery);
    drop(frozen);
    plan.attach_new(selected);

    // Suffixes don't overlap the unevaluated regions, kept prefixes or ungrouped regions,
    // nothing is counted twice.
    // New unions change memberships, so we forget classifications from the first scan.
    let frozen = Frozen::new(selected);
    discovery.forget();
    plan.scan_evaluated(&frozen, &mut discovery);
    drop(frozen);
    plan.attach_new(selected);

    Some(plan)
}

/// Returns `true` when every selected grouped storage is fully evaluated and every group's members
/// hold fully trusted ranges of the same length, regroup then has nothing to do.
///
/// It only reads metadata, it doesn't allocate.
pub(super) fn is_clean(selected: &Selected<'_>) -> bool {
    for storage_id in selected.ids() {
        let storage = selected.get(storage_id);
        let Some(groups) = storage.groups().filter(|groups| groups.has_ranges()) else {
            continue;
        };

        if groups.unevaluated_start != storage.dense().expect("group members are SparseSet").len() {
            return false;
        }

        for range in &groups.ranges {
            if range.trust != range.len {
                return false;
            }

            // Each member compares its length with the first one, their own trust is checked
            // when we get to them.
            let first_id = range.group[0];
            if &first_id == storage_id {
                continue;
            }

            let first_len = selected
                .get(&first_id)
                .groups()
                .and_then(|groups| groups.range(&range.group))
                .expect("every member holds a range for its group")
                .len;

            if first_len != range.len {
                return false;
            }
        }
    }

    true
}
