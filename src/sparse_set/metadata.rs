use super::SparseSet;
use crate::sparse_set::SparseArray;
use crate::storage::{AllStorages, EntityId};
use alloc::vec::Vec;
// #[cfg(feature = "serde1")]
// use crate::storage::Storage;
// #[cfg(feature = "serde1")]
// use super::{SparseSet, SparseSetDeserializer};
// #[cfg(feature = "serde1")]
// use crate::atomic_refcell::AtomicRefCell;
// #[cfg(feature = "serde1")]
// use crate::serde_setup::{GlobalDeConfig, GlobalSerConfig, Identifier, SerConfig};
// #[cfg(feature = "serde1")]
// use hashbrown::HashMap;

pub(crate) const BUCKET_SIZE: usize = 128 / core::mem::size_of::<EntityId>();

pub struct Metadata<T> {
    pub(crate) shared: SparseArray<[EntityId; BUCKET_SIZE]>,
    pub(crate) update: Option<UpdatePack<T>>,
    pub(super) local_on_insert: Vec<fn(EntityId, &mut SparseSet<T>)>,
    pub(super) local_on_remove: Vec<fn(EntityId, &mut SparseSet<T>)>,
    pub(crate) global_on_insert: Vec<fn(EntityId, &mut SparseSet<T>, &AllStorages)>,
    pub(crate) on_insert_ids_sparse: SparseArray<[EntityId; super::BUCKET_SIZE]>,
    pub(crate) on_insert_ids_dense: Vec<EntityId>,
    pub(crate) global_on_remove: Vec<fn(EntityId, &mut SparseSet<T>, &AllStorages)>,
    pub(crate) on_remove_ids_sparse: SparseArray<[EntityId; super::BUCKET_SIZE]>,
    pub(crate) on_remove_ids_dense: Vec<EntityId>,
    // #[cfg(feature = "serde1")]
    // pub(crate) serde: Option<SerdeInfos<T>>,
}

impl<T> Default for Metadata<T> {
    fn default() -> Self {
        Metadata {
            shared: SparseArray::new(),
            update: None,
            local_on_insert: Vec::new(),
            local_on_remove: Vec::new(),
            global_on_insert: Vec::new(),
            on_insert_ids_sparse: SparseArray::new(),
            on_insert_ids_dense: Vec::new(),
            global_on_remove: Vec::new(),
            on_remove_ids_sparse: SparseArray::new(),
            on_remove_ids_dense: Vec::new(),
            // #[cfg(feature = "serde1")]
            // serde: None,
        }
    }
}

pub(crate) struct UpdatePack<T> {
    pub(crate) removed: Vec<EntityId>,
    pub(crate) deleted: Vec<(EntityId, T)>,
}

impl<T> Default for UpdatePack<T> {
    fn default() -> Self {
        UpdatePack {
            removed: Vec::new(),
            deleted: Vec::new(),
        }
    }
}

// #[cfg(feature = "serde1")]
// #[allow(unused)]
// pub(crate) struct SerdeInfos<T> {
//     pub(crate) serialization: fn(
//         &SparseSet<T>,
//         GlobalSerConfig,
//         &mut dyn crate::erased_serde::Serializer,
//     )
//         -> Result<crate::erased_serde::Ok, crate::erased_serde::Error>,
//     pub(crate) deserialization: fn(
//         GlobalDeConfig,
//         &HashMap<EntityId, EntityId>,
//         &mut dyn crate::erased_serde::Deserializer<'_>,
//     ) -> Result<Storage, crate::erased_serde::Error>,
//     pub(crate) with_shared: bool,
//     pub(crate) identifier: Option<Identifier>,
// }

// #[cfg(feature = "serde1")]
// impl<T: serde::Serialize + for<'de> serde::Deserialize<'de> + 'static> SerdeInfos<T> {
//     pub(super) fn new(ser_config: SerConfig) -> Self {
//         SerdeInfos {
//             serialization:
//                 |sparse_set: &SparseSet<T>,
//                  ser_config: GlobalSerConfig,
//                  serializer: &mut dyn crate::erased_serde::Serializer| {
//                     crate::erased_serde::Serialize::erased_serialize(
//                         &super::SparseSetSerializer {
//                             sparse_set: &sparse_set,
//                             ser_config,
//                         },
//                         serializer,
//                     )
//                 },
//             deserialization:
//                 |de_config: GlobalDeConfig,
//                  entities_map: &HashMap<EntityId, EntityId>,
//                  deserializer: &mut dyn crate::erased_serde::Deserializer<'_>| {
//                     #[cfg(feature = "std")]
//                     {
//                         Ok(Storage(Box::new(AtomicRefCell::new(
//                             serde::de::DeserializeSeed::deserialize(
//                                 SparseSetDeserializer::<T> {
//                                     de_config,
//                                     _phantom: core::marker::PhantomData,
//                                 },
//                                 deserializer,
//                             )?,
//                             None,
//                             true,
//                         ))))
//                     }
//                     #[cfg(not(feature = "std"))]
//                     {
//                         Ok(Storage(Box::new(AtomicRefCell::new(
//                             serde::de::DeserializeSeed::deserialize(
//                                 SparseSetDeserializer::<T> {
//                                     de_config,
//                                     _phantom: core::marker::PhantomData,
//                                 },
//                                 deserializer,
//                             )?,
//                         ))))
//                     }
//                 },
//             with_shared: true,
//             identifier: ser_config.identifier,
//         }
//     }
// }
