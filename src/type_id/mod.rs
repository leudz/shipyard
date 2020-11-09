mod hasher;

pub(crate) use hasher::TypeIdHasher;

use core::hash::{Hash, Hasher};

// We have to make our own `TypeId` to be able to deserialize it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct TypeId(u64);

impl TypeId {
    pub(crate) fn of<T: ?Sized + 'static>() -> Self {
        core::any::TypeId::of::<T>().into()
    }
}

impl From<core::any::TypeId> for TypeId {
    fn from(type_id: core::any::TypeId) -> Self {
        let mut hasher = TypeIdHasher::default();

        type_id.hash(&mut hasher);

        TypeId(hasher.finish())
    }
}

impl From<&core::any::TypeId> for TypeId {
    fn from(type_id: &core::any::TypeId) -> Self {
        let mut hasher = TypeIdHasher::default();

        type_id.hash(&mut hasher);

        TypeId(hasher.finish())
    }
}

impl PartialEq<core::any::TypeId> for TypeId {
    fn eq(&self, other: &core::any::TypeId) -> bool {
        let type_id: TypeId = other.into();

        *self == type_id
    }
}

impl PartialEq<TypeId> for core::any::TypeId {
    fn eq(&self, other: &TypeId) -> bool {
        *other == *self
    }
}

// #[cfg(feature = "serde1")]
// impl serde::Serialize for TypeId {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         serializer.serialize_newtype_struct("TypeId", &self.0)
//     }
// }

// #[cfg(feature = "serde1")]
// impl<'de> serde::Deserialize<'de> for TypeId {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         struct TypeIdVisitor;

//         impl<'de> serde::de::Visitor<'de> for TypeIdVisitor {
//             type Value = TypeId;

//             fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//                 formatter.write_str("a type id")
//             }

//             fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
//             where
//                 D: serde::Deserializer<'de>,
//             {
//                 Ok(TypeId(serde::Deserialize::deserialize(deserializer)?))
//             }
//         }

//         deserializer.deserialize_newtype_struct("TypeId", TypeIdVisitor)
//     }
// }
