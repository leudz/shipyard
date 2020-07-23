// This file was modified and is not the same as the one present in erased_serde.

#[macro_use]
mod macros;

mod any;
mod de;
mod error;
mod ser;

pub use de::{deserialize, DeserializeSeed, Deserializer, Visitor};
pub use error::{Error, Result};
pub use ser::{serialize, Ok, Serialize, Serializer};
