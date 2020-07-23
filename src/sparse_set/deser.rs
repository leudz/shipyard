use super::{Metadata, SparseArray, SparseSet};
use crate::serde_setup::{GlobalDeConfig, GlobalSerConfig};
use crate::storage::EntityId;
use core::marker::PhantomData;

#[allow(unused)]
pub(crate) struct SparseSetSerializer<'a, T> {
    pub(crate) sparse_set: &'a SparseSet<T>,
    pub(crate) ser_config: GlobalSerConfig,
}

impl<T> serde::Serialize for SparseSetSerializer<'_, T>
where
    T: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("SparseSet", 2)?;
        state.serialize_field("ids", &self.sparse_set.dense)?;
        state.serialize_field("data", &self.sparse_set.data)?;
        state.end()
    }
}

pub(super) struct SparseSetDeserializer<T> {
    pub(super) de_config: GlobalDeConfig,
    pub(super) _phantom: PhantomData<T>,
}

impl<'de, T> serde::de::DeserializeSeed<'de> for SparseSetDeserializer<T>
where
    T: serde::Deserialize<'de>,
{
    type Value = SparseSet<T>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &'static [&'static str] = &["ids", "data"];

        enum Field {
            Ids,
            Data,
        }

        struct FieldVisitor;
        impl<'de> serde::de::Visitor<'de> for FieldVisitor {
            type Value = Field;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("field identifier")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0u64 => Ok(Field::Ids),
                    1u64 => Ok(Field::Data),
                    _ => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"field index 0 <= i < 2",
                    )),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "ids" => Ok(Field::Ids),
                    "data" => Ok(Field::Data),
                    _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                }
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    b"ids" => Ok(Field::Ids),
                    b"data" => Ok(Field::Data),
                    _ => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Bytes(value),
                        &"field are `ids` and `data`",
                    )),
                }
            }
        }

        impl<'de> serde::Deserialize<'de> for Field {
            #[inline]
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        #[allow(unused)]
        struct Visitor<'de, T>
        where
            T: serde::Deserialize<'de>,
        {
            marker: PhantomData<SparseSet<T>>,
            lifetime: PhantomData<&'de ()>,
            de_config: GlobalDeConfig,
        }

        impl<'de, T> serde::de::Visitor<'de> for Visitor<'de, T>
        where
            T: serde::Deserialize<'de>,
        {
            type Value = SparseSet<T>;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("struct SparseSet")
            }

            #[inline]
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let sparse = SparseArray::new();
                let dense = seq.next_element()?.ok_or_else(|| {
                    serde::de::Error::invalid_length(0, &"struct SparseSet with 2 elements")
                })?;
                let data = seq.next_element()?.ok_or_else(|| {
                    serde::de::Error::invalid_length(1usize, &"struct SparseSet with 2 elements")
                })?;
                let metadata = Metadata::default();

                Ok(SparseSet {
                    sparse,
                    dense,
                    data,
                    metadata,
                })
            }

            #[inline]
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut dense: Option<Vec<EntityId>> = None;
                let mut data: Option<Vec<T>> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Ids => {
                            if dense.is_some() {
                                return Err(serde::de::Error::duplicate_field("ids"));
                            }
                            dense = Some(map.next_value()?);
                        }
                        Field::Data => {
                            if data.is_some() {
                                return Err(serde::de::Error::duplicate_field("data"));
                            }
                            data = Some(map.next_value()?);
                        }
                    }
                }
                let dense = dense.ok_or_else(|| serde::de::Error::missing_field("dense"))?;
                let data = data.ok_or_else(|| serde::de::Error::missing_field("data"))?;

                let mut sparse: SparseArray<[usize; super::BUCKET_SIZE]> = SparseArray::new();
                for (i, &id) in dense.iter().enumerate() {
                    sparse.allocate_at(id);
                    unsafe {
                        sparse.set_sparse_index_unchecked(id, i);
                    }
                }

                Ok(SparseSet {
                    sparse,
                    dense,
                    data,
                    metadata: Default::default(),
                })
            }
        }

        deserializer.deserialize_struct(
            "SparseSet",
            FIELDS,
            Visitor {
                marker: PhantomData::<SparseSet<T>>,
                lifetime: PhantomData,
                de_config: self.de_config,
            },
        )
    }
}
