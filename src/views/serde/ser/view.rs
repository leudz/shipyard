use crate::component::Component;
use crate::iter::IntoIter;
use crate::tracking::Tracking;
use crate::views::View;
use alloc::vec::Vec;
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};

/// Builder to customize [`View`]'s serialization format.
///
/// Make sure to match the configuration when deserializing.
pub struct ViewSerializer<'tmp, 'view, T: Component, Track> {
    #[allow(missing_docs)]
    pub view: &'tmp View<'view, T, Track>,
    /// Serializes the component's type name in addition to the ids and components.
    pub type_names: bool,
}

impl<'tmp, 'view, T: Component, Track> ViewSerializer<'tmp, 'view, T, Track> {
    #[allow(missing_docs)]
    pub fn new(view: &'tmp View<'view, T, Track>) -> ViewSerializer<'tmp, 'view, T, Track> {
        ViewSerializer {
            view,
            type_names: false,
        }
    }

    /// Serializes the component's type name in addition to the ids and components.
    pub fn type_names(&mut self, type_names: bool) -> &mut Self {
        self.type_names = type_names;

        self
    }
}

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

impl<'tmp, 'view, T: Component + Serialize, Track: Tracking> Serialize
    for ViewSerializer<'tmp, 'view, T, Track>
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.type_names {
            use serde::ser::SerializeStruct as _;

            let mut state = serializer.serialize_struct("View", 2)?;
            state.serialize_field("type_name", &core::any::type_name::<T>())?;

            let data = self.view.iter().with_id().collect::<Vec<_>>();
            state.serialize_field("data", &data)?;

            state.end()
        } else {
            let mut seq = serializer.serialize_seq(Some(self.view.len()))?;

            self.view
                .iter()
                .with_id()
                .try_for_each(|(eid, component)| seq.serialize_element(&(eid, component)))?;

            seq.end()
        }
    }
}
