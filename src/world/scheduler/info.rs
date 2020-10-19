//! Contains types for displaying workload information.

use crate::borrow::Mutability;
use crate::storage::StorageId;
use crate::type_id::TypeId;
use alloc::borrow::Cow;
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub struct WorkloadInfo {
    pub name: Cow<'static, str>,
    pub batch_info: Vec<BatchInfo>,
}

#[derive(Debug, Clone)]
pub struct BatchInfo {
    pub systems: Vec<SystemInfo>,
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub name: &'static str,
    pub type_id: TypeId,
    pub borrow: Vec<TypeInfo>,
    pub conflict: Option<Conflict>,
}

#[derive(Debug, Clone)]
pub enum Conflict {
    Borrow {
        system: SystemId,
        type_info: TypeInfo,
    },
    NotSendSync,
}

#[derive(Debug, Clone)]
pub struct SystemId {
    pub name: &'static str,
    pub type_id: TypeId,
}

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
