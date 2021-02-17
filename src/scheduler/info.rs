//! Types for displaying workload information.

pub use crate::type_id::TypeId;

use crate::borrow::Mutability;
use crate::storage::StorageId;
use alloc::borrow::Cow;
use alloc::vec::Vec;

/// Contains information related to a workload.
///
/// A workload is a collection of systems with parallelism calculated based on the types borrow by the systems.
#[derive(Debug, Clone)]
pub struct WorkloadInfo {
    #[allow(missing_docs)]
    pub name: Cow<'static, str>,
    #[allow(missing_docs)]
    pub batch_info: Vec<BatchInfo>,
}

/// Contains information related to a batch.
///
/// A batch is a collection of system that can safely run in parallel.
#[derive(Debug, Clone)]
pub struct BatchInfo {
    #[allow(missing_docs)]
    pub systems: Vec<SystemInfo>,
}

/// Contains information related to a system.
#[derive(Clone)]
pub struct SystemInfo {
    #[allow(missing_docs)]
    pub name: &'static str,
    #[allow(missing_docs)]
    pub type_id: TypeId,
    #[allow(missing_docs)]
    pub borrow: Vec<TypeInfo>,
    /// Information explaining why this system could not be part of the previous batch.
    pub conflict: Option<Conflict>,
}

impl core::fmt::Debug for SystemInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StorageInfo")
            .field("name", &self.name)
            .field("borrow", &self.borrow)
            .field("conflict", &self.conflict)
            .finish()
    }
}

/// Pinpoints the type and system that made a system unable to get into a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conflict {
    /// Rust rules do not allow the type described by 'type_info' to be borrowed at the same time as 'other_type_info'.
    Borrow {
        #[allow(missing_docs)]
        type_info: TypeInfo,
        #[allow(missing_docs)]
        other_system: SystemId,
        #[allow(missing_docs)]
        other_type_info: TypeInfo,
    },
    /// A '!Send' and/or '!Sync' type currently prevents any parrallelism.
    NotSendSync(TypeInfo),
    /// A '!Send' and/or '!Sync' type currently prevents any parrallelism.
    OtherNotSendSync {
        #[allow(missing_docs)]
        system: SystemId,
        #[allow(missing_docs)]
        type_info: TypeInfo,
    },
}

/// Identify a system.
#[derive(Clone, Eq)]
pub struct SystemId {
    #[allow(missing_docs)]
    pub name: &'static str,
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
        f.write_str(self.name)
    }
}

/// Identify a type.
#[derive(Clone, Eq)]
pub struct TypeInfo {
    #[allow(missing_docs)]
    pub name: &'static str,
    #[allow(missing_docs)]
    pub mutability: Mutability,
    #[allow(missing_docs)]
    pub storage_id: StorageId,
    #[allow(missing_docs)]
    pub is_send: bool,
    #[allow(missing_docs)]
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
        let mut debug_struct = f.debug_struct("TypeInfo");

        match (self.is_send, self.is_sync) {
            (true, true) => debug_struct.field("name", &self.name),
            (false, true) => {
                debug_struct.field("name", &format_args!("shipyard::NonSend<{}>", self.name))
            }
            (true, false) => {
                debug_struct.field("name", &format_args!("shipyard::NonSync<{}>", self.name))
            }
            (false, false) => debug_struct.field(
                "name",
                &format_args!("shipyard::NonSendSync<{}>", self.name),
            ),
        }
        .field("mutability", &self.mutability)
        .finish()
    }
}
