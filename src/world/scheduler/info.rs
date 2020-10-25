//! Contains types for displaying workload information.

use crate::borrow::Mutability;
use crate::storage::StorageId;
use crate::type_id::TypeId;
use alloc::borrow::Cow;
use alloc::vec::Vec;

/// Contains information related to a workload.
///
/// A workload is a collection of systems with parallelism calculated based on the types borrow by the systems.
#[derive(Debug, Clone)]
pub struct WorkloadInfo {
    pub name: Cow<'static, str>,
    pub batch_info: Vec<BatchInfo>,
}

/// Contains information related to a batch.
///
/// A batch is a collection of system that can safely run in parallel.
#[derive(Debug, Clone)]
pub struct BatchInfo {
    pub systems: Vec<SystemInfo>,
}

/// Contains information related to a system.
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub name: &'static str,
    pub type_id: TypeId,
    pub borrow: Vec<TypeInfo>,
    pub conflict: Option<Conflict>,
}

/// Pinpoints the type and system that made a system unable to get into a batch.
#[derive(Debug, Clone)]
pub enum Conflict {
    Borrow {
        system: SystemId,
        type_info: TypeInfo,
    },
    NotSendSync,
}

/// Identify a system.
#[derive(Debug, Clone)]
pub struct SystemId {
    pub name: &'static str,
    pub type_id: TypeId,
}

/// Identify a type.
#[derive(Clone, Eq)]
pub struct TypeInfo {
    pub name: &'static str,
    pub mutability: Mutability,
    pub storage_id: StorageId,
    pub is_send: bool,
    pub is_sync: bool,
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

impl core::fmt::Debug for TypeInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("TypeInfo {\n")?;
        match (self.is_send, self.is_sync) {
            (true, true) => f.write_fmt(format_args!("    name: {:?}\n", self.name))?,
            (false, true) => f.write_fmt(format_args!(
                "    name: \"shipyard::NonSend<{}>\"\n",
                self.name
            ))?,
            (true, false) => f.write_fmt(format_args!(
                "    name: \"shipyard::NonSync<{}>\"\n",
                self.name
            ))?,
            (false, false) => f.write_fmt(format_args!(
                "    name: \"shipyard::NonSendSync<{}>\"\n",
                self.name
            ))?,
        }
        f.write_fmt(format_args!("    mutability: {:?}\n", self.mutability))?;
        f.write_fmt(format_args!("    storage_id: {:?}\n", self.storage_id))?;
        f.write_str("}")
    }
}
