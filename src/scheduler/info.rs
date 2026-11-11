//! Types for displaying workload information.

use crate::ShipHashMap;
use crate::borrow::Mutability;
use crate::scheduler::{AsLabel, Label};
use crate::storage::StorageId;
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::TypeId;

/// Contains information related to a workload.
///
/// A workload is a collection of systems with parallelism calculated based on the types borrow by the systems.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct WorkloadInfo {
    #[allow(missing_docs)]
    pub name: String,
    #[allow(missing_docs)]
    pub batches_info: Vec<BatchInfo>,
}

/// Contains information related to a batch.
///
/// A batch is a collection of systems, and regroups in registered workloads, that can safely run in parallel.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct BatchInfo {
    #[allow(missing_docs)]
    pub systems: (Option<SystemInfo>, Vec<SystemInfo>),
    /// Regroups running in this batch, alongside its systems.
    pub regroups: Vec<RegroupInfo>,
}

impl BatchInfo {
    /// Returns an iterator of all systems in this batch
    pub fn systems(&self) -> impl Iterator<Item = &'_ SystemInfo> {
        self.systems.0.iter().chain(&self.systems.1)
    }
}

/// Contains information related to a system.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct SystemInfo {
    #[allow(missing_docs)]
    pub name: String,
    #[allow(missing_docs)]
    pub borrow: Vec<TypeInfo>,
    /// Information explaining why this system could not be part of the previous batch.
    ///
    /// Only one reason is kept and it names the other system by callable index, see `unique_id`,
    /// which doesn't tell its occurrences apart. [`SystemOccurrence`] has the facts behind every reason.
    pub conflict: Option<Conflict>,
    /// The systems this one has to run after, in an earlier batch, by [`SystemOccurrence::id`].
    ///
    /// Regroups aren't listed. [`SystemOccurrence::after`] lists them too, with why each one is there.
    pub after: Vec<usize>,
    /// The `after_all` constraints that ordered this system, `other_system` being a [`SystemOccurrence::id`].
    ///
    /// Only the first matching label of each other system is listed, [`SystemOccurrence::after`] has all of them.
    pub after_all: Vec<BeforeAfterConstraint>,
    /// The `before_all` constraints that ordered this system, `other_system` being a callable index,
    /// see `unique_id`.
    ///
    /// Only the first matching label of each other system is listed,
    /// the other system's [`SystemOccurrence::after`] has all of them.
    pub before_all: Vec<BeforeAfterConstraint>,
    /// Index of the system's callable in the world's scheduler.
    ///
    /// Every occurrence of a system shares it, in this workload and in the others.
    /// [`SystemOccurrence::id`] tells occurrences apart.
    pub unique_id: usize,
    /// What the scheduler used to place this appearance of the system, see [`SystemOccurrence`].
    ///
    /// Unlike `unique_id`, shared by every appearance of a system, it tells them apart.
    pub occurrence: SystemOccurrence,
}

impl core::fmt::Debug for SystemInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SystemInfo")
            .field("name", &self.name)
            .field("unique_id", &self.unique_id)
            .field("borrow", &self.borrow)
            .field("conflict", &self.conflict)
            .field("after", &self.after)
            .field("after_all", &self.after_all)
            .field("before_all", &self.before_all)
            .field("occurrence", &self.occurrence)
            .finish()
    }
}

/// One appearance of a system in a workload, and what the scheduler used to place it.
///
/// A system can be added to a workload several times, each time is an occurrence.
/// Occurrences of the same system run the same function but are scheduled on their own:
/// each has its own conditions, its own ordering, its own batch and its own [`SystemInfo`].
///
/// Occurrences and regroups are what the scheduler places in batches,
/// [`OccurrenceRef`] points to one of them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct SystemOccurrence {
    /// Identifies the occurrence within its workload, see [`OccurrenceRef::System`].
    ///
    /// It's the occurrence's position among the workload's systems, in the order they were added,
    /// nested workloads included. Two occurrences of the same system have different ids.
    pub id: usize,
    /// What the system's conditions borrow. [`SystemInfo::borrow`] is what the system itself borrows.
    pub conditions: Vec<TypeInfo>,
    /// What the scheduler compares to place the occurrence: what the system and its conditions borrow.
    ///
    /// A storage borrowed several times is listed once, exclusive if one of the borrows is,
    /// not thread safe if one of them isn't.
    pub footprint: Vec<TypeInfo>,
    /// Whether the occurrence runs on the calling thread, a batch can only have one such job.
    pub thread_bound: bool,
    /// The occurrences that have to run in an earlier batch, sorted, each listed once.
    pub after: Vec<Predecessor>,
}

/// What the scheduler used to place one regroup of a workload.
///
/// Regroups are placed like [system occurrences](SystemOccurrence),
/// two regroups of the same storages are placed on their own.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct RegroupOccurrence {
    /// Identifies the regroup within its workload, see [`OccurrenceRef::Regroup`].
    ///
    /// Two regroups of the same storages have different ids.
    pub id: usize,
    /// Whether the regroup runs on the calling thread, a batch can only have one such job.
    pub thread_bound: bool,
    /// The occurrences that have to run in an earlier batch, sorted, each listed once.
    pub after: Vec<Predecessor>,
}

/// Points to a system occurrence or a regroup of the same workload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub enum OccurrenceRef {
    /// A [`SystemOccurrence::id`].
    System(usize),
    /// A [`RegroupOccurrence::id`].
    Regroup(usize),
}

/// An occurrence that has to run in an earlier batch, and every reason why.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct Predecessor {
    #[allow(missing_docs)]
    pub occurrence: OccurrenceRef,
    /// Never empty, sorted, each listed once.
    pub origins: Vec<OrderingOrigin>,
}

/// Why an occurrence has to run in an earlier batch than another.
///
/// Only hard ordering is listed, the sequential order alone doesn't keep two systems apart.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub enum OrderingOrigin {
    /// The predecessor comes first in the workload and they can't run at the same time.
    Implicit,
    /// A barrier separates them in the workload.
    Barrier,
    /// The predecessor is `before_all` this label, which the successor has.
    BeforeAll(String),
    /// The successor is `after_all` this label, which the predecessor has.
    AfterAll(String),
    /// They come from consecutive children of a sequential workload.
    Sequential,
    /// The predecessor's `WorkloadSystem::before` lists the successor.
    Before,
    /// The successor borrows from the storages the predecessor regroup moves.
    Regroup,
    /// The successor regroup moves storages the predecessor might have changed since the set's
    /// previous regroup, the regroup is there for it.
    Writes,
    /// The successor regroup moves storages the predecessor borrows, without having changed them
    /// since the set's previous regroup.
    Borrows,
}

/// Contains information related to a regroup a workload runs between its systems.
///
/// The workload adds these itself, they aren't systems and can't be named.\
/// A regroup runs on the world's thread when one of its storages isn't `thread_safe`,
/// like a system in [`BatchInfo::systems`]'s first slot.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct RegroupInfo {
    /// Every storage of the regrouped connected set, all borrowed exclusively.
    pub borrow: Vec<TypeInfo>,
    /// What makes the regroup run.
    pub trigger: RegroupTrigger,
    /// What the scheduler used to place this regroup.
    pub occurrence: RegroupOccurrence,
}

/// Why a workload has a regroup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub enum RegroupTrigger {
    /// Runs each time the workload starts, for the changes made outside of it.
    Entry,
    /// Placed after a system borrowing a member of the set exclusively.
    ///
    /// Like every regroup it runs each time the workload reaches it, a set with nothing to move is cheap.
    Pending,
}

/// Pinpoints the type and system that made a system unable to get into a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub enum Conflict {
    /// Rust rules do not allow the type described by `type_info` to be borrowed at the same time as `other_type_info`.
    Borrow {
        #[allow(missing_docs)]
        type_info: TypeInfo,
        /// The other system's callable index, see [`SystemInfo::unique_id`].
        other_system: usize,
        #[allow(missing_docs)]
        other_type_info: TypeInfo,
    },
    /// A `!Send` and/or `!Sync` type currently prevents any parrallelism.
    NotSendSync(TypeInfo),
    /// A `!Send` and/or `!Sync` type currently prevents any parrallelism.
    OtherNotSendSync {
        /// The other system's callable index, see [`SystemInfo::unique_id`].
        system: usize,
        #[allow(missing_docs)]
        type_info: TypeInfo,
    },
    /// A regroup of the previous batch borrows the storage described by this `TypeInfo` exclusively,
    /// the system waits for it.
    Regroup(TypeInfo),
}

/// Identify a type.
#[derive(Clone, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeInfo {
    #[allow(missing_docs)]
    pub name: Cow<'static, str>,
    #[allow(missing_docs)]
    pub mutability: Mutability,
    #[allow(missing_docs)]
    #[cfg_attr(feature = "serde1", serde(skip))]
    pub storage_id: StorageId,
    #[allow(missing_docs)]
    pub thread_safe: bool,
}

impl PartialEq for TypeInfo {
    fn eq(&self, rhs: &Self) -> bool {
        self.storage_id == rhs.storage_id && self.mutability == rhs.mutability
    }
}

impl PartialEq<(TypeId, Mutability)> for TypeInfo {
    fn eq(&self, rhs: &(TypeId, Mutability)) -> bool {
        self.storage_id == rhs.0 && self.mutability == rhs.1
    }
}

impl PartialOrd for TypeInfo {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TypeInfo {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        match self.storage_id.cmp(&other.storage_id) {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        self.mutability.cmp(&other.mutability)
    }
}

impl core::fmt::Debug for TypeInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug_struct = f.debug_struct("TypeInfo");

        debug_struct
            .field("name", &self.name)
            .field("mutability", &self.mutability)
            .field("thread_safe", &self.thread_safe)
            .finish()
    }
}

impl core::hash::Hash for TypeInfo {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.storage_id.hash(state);
        self.mutability.hash(state);
    }
}

/// Contains a list of workloads, their systems and which storages these systems borrow.
#[derive(Default, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct WorkloadsInfo(pub ShipHashMap<String, WorkloadInfo>);

impl WorkloadsInfo {
    /// Creates an empty [`WorkloadsInfo`].
    pub fn new() -> WorkloadsInfo {
        WorkloadsInfo(ShipHashMap::new())
    }
}

/// List of before/after requirements for a system or workload.\
/// The list dedups items.
#[derive(Clone, Debug, Default)]
pub struct DedupedLabels(Vec<Box<dyn Label>>);

impl DedupedLabels {
    pub(crate) fn new() -> DedupedLabels {
        DedupedLabels(Vec::new())
    }

    pub(crate) fn with_capacity(capacity: usize) -> DedupedLabels {
        DedupedLabels(Vec::with_capacity(capacity))
    }

    /// Returns `true` if the `Label` was not already present.
    pub(crate) fn add<T>(&mut self, label: impl AsLabel<T>) -> bool {
        let label = label.as_label();

        // Can't use binary search here as Label can't be ordered
        if !self.0.contains(&label) {
            self.0.push(label);

            true
        } else {
            false
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn iter(&self) -> RequirementsIter<'_> {
        self.into_iter()
    }

    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    pub(crate) fn retain<F: FnMut(&Box<dyn Label>) -> bool>(&mut self, f: F) {
        self.0.retain(f);
    }
}

impl IntoIterator for DedupedLabels {
    type Item = Box<dyn Label>;
    type IntoIter = alloc::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a DedupedLabels {
    type Item = &'a Box<dyn Label>;
    type IntoIter = RequirementsIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        RequirementsIter(self.0.iter())
    }
}

/// Iterator for requirements
pub struct RequirementsIter<'a>(core::slice::Iter<'a, Box<dyn Label>>);

impl<'a> Iterator for RequirementsIter<'a> {
    type Item = &'a Box<dyn Label>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl Extend<Box<dyn Label>> for DedupedLabels {
    fn extend<T: IntoIterator<Item = Box<dyn Label>>>(&mut self, iter: T) {
        for label in iter {
            self.add(label);
        }
    }
}

impl<'a> Extend<&'a Box<dyn Label>> for DedupedLabels {
    fn extend<T: IntoIterator<Item = &'a Box<dyn Label>>>(&mut self, iter: T) {
        for label in iter {
            self.add(label.clone());
        }
    }
}

/// Represents a pair of constraints-tag that matched during scheduling.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct BeforeAfterConstraint {
    /// The other system, see [`SystemInfo::after_all`] and [`SystemInfo::before_all`] for what the number is.
    pub other_system: usize,
    #[allow(missing_docs)]
    pub constraint: String,
}
