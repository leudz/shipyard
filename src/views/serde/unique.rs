use crate::component::Unique;
use crate::views::UniqueView;
use serde::{Serialize, Serializer};

impl<'a, T: Unique + Serialize> Serialize for UniqueView<'a, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.unique.value.serialize(serializer)
    }
}
