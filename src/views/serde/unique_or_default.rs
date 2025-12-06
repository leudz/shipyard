use crate::component::Unique;
use crate::views::UniqueOrDefaultView;
use serde::{Serialize, Serializer};

impl<'a, T: Unique + Default + Serialize> Serialize for UniqueOrDefaultView<'a, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (**self).serialize(serializer)
    }
}
