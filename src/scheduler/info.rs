//! Types for displaying workload information.

use crate::borrow::Mutability;
use crate::scheduler::{AsLabel, Label};
use crate::storage::StorageId;
pub use crate::type_id::TypeId;
use crate::ShipHashMap;
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// Contains information related to a workload.
///
/// A workload is a collection of systems with parallelism calculated based on the types borrow by the systems.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct WorkloadInfo {
    #[allow(missing_docs)]
    pub name: String,
    #[allow(missing_docs)]
    pub batch_info: Vec<BatchInfo>,
}

/// Contains information related to a batch.
///
/// A batch is a collection of system that can safely run in parallel.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct BatchInfo {
    #[allow(missing_docs)]
    pub systems: (Option<SystemInfo>, Vec<SystemInfo>),
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
    pub type_id: TypeId,
    #[allow(missing_docs)]
    pub borrow: Vec<TypeInfo>,
    /// Information explaining why this system could not be part of the previous batch.
    pub conflict: Option<Conflict>,
    #[allow(missing_docs)]
    pub before: Vec<String>,
    #[allow(missing_docs)]
    pub after: Vec<String>,
}

impl core::fmt::Debug for SystemInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SystemInfo")
            .field("name", &self.name)
            .field("borrow", &self.borrow)
            .field("conflict", &self.conflict)
            .finish()
    }
}

/// Pinpoints the type and system that made a system unable to get into a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub enum Conflict {
    /// Rust rules do not allow the type described by `type_info` to be borrowed at the same time as `other_type_info`.
    Borrow {
        #[allow(missing_docs)]
        type_info: Option<TypeInfo>,
        #[allow(missing_docs)]
        other_system: SystemId,
        #[allow(missing_docs)]
        other_type_info: TypeInfo,
    },
    /// A `!Send` and/or `!Sync` type currently prevents any parrallelism.
    NotSendSync(TypeInfo),
    /// A `!Send` and/or `!Sync` type currently prevents any parrallelism.
    OtherNotSendSync {
        #[allow(missing_docs)]
        system: SystemId,
        #[allow(missing_docs)]
        type_info: TypeInfo,
    },
}

/// Identify a system.
#[derive(Clone, Eq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct SystemId {
    #[allow(missing_docs)]
    pub name: String,
    #[allow(missing_docs)]
    pub type_id: TypeId,
}

impl PartialEq for SystemId {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id
    }
}

impl core::fmt::Debug for SystemId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{:?}", self.name))
    }
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

/// List of before/after requirements for a system or workload.
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

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    pub(crate) fn to_vec(&self) -> Vec<Box<dyn Label>> {
        self.0.clone()
    }

    pub(crate) fn retain<F: FnMut(&Box<dyn Label>) -> bool>(&mut self, f: F) {
        self.0.retain(f);
    }

    pub(crate) fn to_string_vec(&self) -> Vec<String> {
        self.0.iter().map(|label| format!("{:?}", label)).collect()
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
