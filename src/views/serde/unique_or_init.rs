use crate::component::Unique;
use crate::views::UniqueOrInitView;
use serde::{Serialize, Serializer};

impl<'a, T: Unique + Send + Sync + Serialize> Serialize for UniqueOrInitView<'a, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.get().serialize(serializer)
    }
}
