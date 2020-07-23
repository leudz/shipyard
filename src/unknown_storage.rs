#[cfg(feature = "serde1")]
use crate::serde_setup::{GlobalDeConfig, GlobalSerConfig, ANCHOR};
use crate::sparse_set::SparseSet;
#[cfg(feature = "serde1")]
use crate::storage::Storage;
use crate::storage::{Entities, EntityId};
use crate::type_id::TypeId;
use alloc::vec::Vec;
use core::any::Any;

pub(super) trait UnknownStorage {
    fn delete(&mut self, entity: EntityId, storage_to_unpack: &mut Vec<TypeId>);
    fn clear(&mut self);
    fn unpack(&mut self, entity: EntityId);
    fn any(&self) -> &dyn Any;
    fn any_mut(&mut self) -> &mut dyn Any;
    #[cfg(feature = "serde1")]
    fn is_serializable(&self) -> bool {
        false
    }
    #[cfg(feature = "serde1")]
    fn skip_serialization(&self, _: GlobalSerConfig) -> bool {
        true
    }
    #[cfg(feature = "serde1")]
    fn serialize(
        &self,
        _: GlobalSerConfig,
        _: &mut dyn crate::erased_serde::Serializer,
    ) -> crate::erased_serde::Result<crate::erased_serde::Ok> {
        assert!(
            !self.is_serializable(),
            "UnknownStorage's (de)ser impl is incorrect."
        );
        Err(serde::ser::Error::custom("This type isn't serializable."))
    }
    #[cfg(feature = "serde1")]
    fn deserialize(
        &self,
    ) -> Option<
        fn(
            GlobalDeConfig,
            &mut dyn crate::erased_serde::Deserializer<'_>,
        ) -> Result<Storage, crate::erased_serde::Error>,
    > {
        assert!(
            !self.is_serializable(),
            "UnknownStorage's (de)ser impl is incorrect."
        );
        None
    }
}

impl dyn UnknownStorage {
    pub(crate) fn sparse_set<T: 'static>(&self) -> Option<&SparseSet<T>> {
        self.any().downcast_ref()
    }
    pub(crate) fn sparse_set_mut<T: 'static>(&mut self) -> Option<&mut SparseSet<T>> {
        self.any_mut().downcast_mut()
    }
    pub(crate) fn entities(&self) -> Option<&Entities> {
        self.any().downcast_ref()
    }
    pub(crate) fn entities_mut(&mut self) -> Option<&mut Entities> {
        self.any_mut().downcast_mut()
    }
    pub(crate) fn unique<T: 'static>(&self) -> Option<&T> {
        self.any().downcast_ref()
    }
    pub(crate) fn unique_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.any_mut().downcast_mut()
    }
}

#[cfg(feature = "serde1")]
pub(crate) fn deserialize_ptr(
    func: fn(
        GlobalDeConfig,
        &mut dyn crate::erased_serde::Deserializer<'_>,
    ) -> Result<Storage, crate::erased_serde::Error>,
) -> usize {
    let anchor: *const () = &ANCHOR;
    anchor as usize - func as usize
}

#[cfg(feature = "serde1")]
pub(crate) unsafe fn deserialize_fn(
    ptr: usize,
) -> fn(
    GlobalDeConfig,
    &mut dyn crate::erased_serde::Deserializer<'_>,
) -> Result<Storage, crate::erased_serde::Error> {
    let anchor: *const () = &ANCHOR;
    let deserialize_ptr = anchor as usize - ptr;
    let deserialize: *const () = deserialize_ptr as *const _;
    core::mem::transmute(deserialize)
}

#[cfg(feature = "serde1")]
pub(crate) struct StorageSerializer<'a> {
    pub(crate) unknown_storage: &'a dyn UnknownStorage,
    pub(crate) ser_config: GlobalSerConfig,
}

#[cfg(feature = "serde1")]
impl serde::Serialize for StorageSerializer<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.unknown_storage
            .serialize(
                self.ser_config,
                &mut crate::erased_serde::Serializer::erase(serializer),
            )
            .map(crate::erased_serde::Ok::take)
            .map_err(serde::ser::Error::custom)
    }
}
