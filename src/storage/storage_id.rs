use crate::type_id::TypeId;

/// Id of a storage, can be a `TypeId` or a user defined `u64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StorageId {
    TypeId(TypeId),
    Custom(u64),
}

impl StorageId {
    pub fn of<T: 'static>() -> Self {
        TypeId::of::<T>().into()
    }
}

impl From<TypeId> for StorageId {
    fn from(type_id: TypeId) -> Self {
        StorageId::TypeId(type_id)
    }
}

impl From<core::any::TypeId> for StorageId {
    fn from(type_id: core::any::TypeId) -> Self {
        StorageId::TypeId(type_id.into())
    }
}

impl From<u64> for StorageId {
    fn from(int: u64) -> Self {
        StorageId::Custom(int)
    }
}

// #[cfg(feature = "serde1")]
// impl serde::Serialize for StorageId {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         match *self {
//             StorageId::TypeId(type_id) => {
//                 serializer.serialize_newtype_variant("StorageId", 0u32, "TypeId", &type_id)
//             }
//             StorageId::Custom(custom) => {
//                 serializer.serialize_newtype_variant("StorageId", 1u32, "Custom", &custom)
//             }
//         }
//     }
// }

// #[cfg(feature = "serde1")]
// impl<'de> serde::Deserialize<'de> for StorageId {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         const VARIANTS: &'static [&'static str] = &["TypeId", "Custom"];

//         enum Field {
//             TypeId,
//             Custom,
//         }

//         struct FieldVisitor;

//         impl<'de> serde::de::Visitor<'de> for FieldVisitor {
//             type Value = Field;

//             fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//                 formatter.write_str("variant identifier")
//             }

//             fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
//             where
//                 E: serde::de::Error,
//             {
//                 match value {
//                     0u64 => Ok(Field::TypeId),
//                     1u64 => Ok(Field::Custom),
//                     _ => Err(serde::de::Error::invalid_value(
//                         serde::de::Unexpected::Unsigned(value),
//                         &"variant index 0 <= i < 2",
//                     )),
//                 }
//             }

//             fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
//             where
//                 E: serde::de::Error,
//             {
//                 match value {
//                     "TypeId" => Ok(Field::TypeId),
//                     "Custom" => Ok(Field::Custom),
//                     _ => Err(serde::de::Error::unknown_variant(value, VARIANTS)),
//                 }
//             }

//             fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
//             where
//                 E: serde::de::Error,
//             {
//                 match value {
//                     b"TypeId" => Ok(Field::TypeId),
//                     b"Custom" => Ok(Field::Custom),
//                     _ => {
//                         let value = &alloc::string::String::from_utf8_lossy(value);
//                         Err(serde::de::Error::unknown_variant(value, VARIANTS))
//                     }
//                 }
//             }
//         }

//         impl<'de> serde::Deserialize<'de> for Field {
//             #[inline]
//             fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//             where
//                 D: serde::Deserializer<'de>,
//             {
//                 serde::Deserializer::deserialize_identifier(deserializer, FieldVisitor)
//             }
//         }

//         struct StorageIdVisitor;

//         impl<'de> serde::de::Visitor<'de> for StorageIdVisitor {
//             type Value = StorageId;

//             fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//                 formatter.write_str("enum StorageId")
//             }

//             fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
//             where
//                 A: serde::de::EnumAccess<'de>,
//             {
//                 match serde::de::EnumAccess::variant(data)? {
//                     (Field::TypeId, variant) => {
//                         serde::de::VariantAccess::newtype_variant::<TypeId>(variant)
//                             .map(StorageId::TypeId)
//                     }
//                     (Field::Custom, variant) => {
//                         serde::de::VariantAccess::newtype_variant::<u64>(variant)
//                             .map(StorageId::Custom)
//                     }
//                 }
//             }
//         }

//         deserializer.deserialize_enum("StorageId", VARIANTS, StorageIdVisitor)
//     }
// }
