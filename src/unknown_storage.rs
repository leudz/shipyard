// #[cfg(feature = "serde1")]
// use crate::serde_setup::{GlobalDeConfig, GlobalSerConfig, ANCHOR};
// #[cfg(feature = "serde1")]
// use crate::storage::Storage;
use crate::storage::EntityId;
use crate::type_id::TypeId;
// #[cfg(feature = "serde1")]
// use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::any::Any;
// #[cfg(feature = "serde1")]
// use hashbrown::HashMap;

pub trait SizedAny {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: 'static> SizedAny for T {
    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub trait UnknownStorage: SizedAny {
    fn any(&self) -> &dyn Any {
        SizedAny::as_any(self)
    }
    fn any_mut(&mut self) -> &mut dyn Any {
        SizedAny::as_any_mut(self)
    }
    #[inline]
    fn delete(&mut self, _: EntityId, _: &mut Vec<TypeId>) {}
    #[inline]
    fn clear(&mut self) {}
    #[inline]
    fn unpack(&mut self, _: EntityId) {}
    #[inline]
    fn share(&mut self, _: EntityId, _: EntityId) {}
    // #[cfg(feature = "serde1")]
    // fn should_serialize(&self, _: GlobalSerConfig) -> bool {
    //     false
    // }
    // #[cfg(feature = "serde1")]
    // fn serialize(
    //     &self,
    //     ser_config: GlobalSerConfig,
    //     _: &mut dyn crate::erased_serde::Serializer,
    // ) -> crate::erased_serde::Result<crate::erased_serde::Ok> {
    //     assert!(
    //         self.should_serialize(ser_config),
    //         "UnknownStorage's (de)ser impl is incorrect."
    //     );
    //     Err(serde::ser::Error::custom("This type isn't serializable."))
    // }
    // #[cfg(feature = "serde1")]
    // fn serialize_identifier(&self) -> Cow<'static, str> {
    //     "".into()
    // }
    // #[cfg(feature = "serde1")]
    // fn deserialize(
    //     &self,
    // ) -> Option<
    //     fn(
    //         GlobalDeConfig,
    //         &HashMap<EntityId, EntityId>,
    //         &mut dyn crate::erased_serde::Deserializer<'_>,
    //     ) -> Result<Storage, crate::erased_serde::Error>,
    // > {
    //     None
    // }
}

// #[cfg(feature = "serde1")]
// pub(crate) fn deserialize_ptr(
//     func: fn(
//         GlobalDeConfig,
//         &mut dyn crate::erased_serde::Deserializer<'_>,
//     ) -> Result<Storage, crate::erased_serde::Error>,
// ) -> usize {
//     let anchor: *const () = &ANCHOR;
//     anchor as usize - func as usize
// }

// #[cfg(feature = "serde1")]
// pub(crate) unsafe fn deserialize_fn(
//     ptr: usize,
// ) -> fn(
//     GlobalDeConfig,
//     &mut dyn crate::erased_serde::Deserializer<'_>,
// ) -> Result<Storage, crate::erased_serde::Error> {
//     let anchor: *const () = &ANCHOR;
//     let deserialize_ptr = anchor as usize - ptr;
//     let deserialize: *const () = deserialize_ptr as *const _;
//     core::mem::transmute(deserialize)
// }

// #[cfg(feature = "serde1")]
// pub(crate) struct StorageSerializer<'a> {
//     pub(crate) unknown_storage: &'a dyn UnknownStorage,
//     pub(crate) ser_config: GlobalSerConfig,
// }

// #[cfg(feature = "serde1")]
// impl serde::Serialize for StorageSerializer<'_> {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         self.unknown_storage
//             .serialize(
//                 self.ser_config,
//                 &mut crate::erased_serde::Serializer::erase(serializer),
//             )
//             .map(crate::erased_serde::Ok::take)
//             .map_err(serde::ser::Error::custom)
//     }
// }
