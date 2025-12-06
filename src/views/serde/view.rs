use crate::component::Component;
use crate::iter::IntoIter;
use crate::tracking::Tracking;
use crate::views::View;
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};

impl<'a, T: Component + Serialize, Track: Tracking> Serialize for View<'a, T, Track> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;

        self.iter()
            .with_id()
            .try_for_each(|(eid, component)| seq.serialize_element(&(eid, component)))?;

        seq.end()
    }
}
