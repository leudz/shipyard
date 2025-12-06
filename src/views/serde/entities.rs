use crate::views::EntitiesView;
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};

impl<'a> Serialize for EntitiesView<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(None)?;

        self.iter()
            .try_for_each(|eid| seq.serialize_element(&eid))?;

        seq.end()
    }
}
