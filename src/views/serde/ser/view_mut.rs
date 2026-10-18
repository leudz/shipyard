use crate::component::Component;
use crate::tracking::Tracking;
use crate::views::ViewMut;
use crate::views::serde::ser::view::ViewSerializer;
use serde::{Serialize, Serializer};

/// Builder to customize [`ViewMut`]'s serialization format.
///
/// Make sure to match the configuration when deserializing.
pub struct ViewMutSerializer<'tmp, 'view, T: Component, Track> {
    #[allow(missing_docs)]
    pub view: &'tmp ViewMut<'view, T, Track>,
    /// Serializes the component's type name in addition to the ids and components.
    pub type_names: bool,
}

impl<'tmp, 'view, T: Component, Track: Tracking> ViewMutSerializer<'tmp, 'view, T, Track> {
    #[allow(missing_docs)]
    pub fn new(view: &'tmp ViewMut<'view, T, Track>) -> ViewMutSerializer<'tmp, 'view, T, Track> {
        ViewMutSerializer {
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

impl<'tmp, 'view, T: Component + Serialize, Track: Tracking> Serialize
    for ViewMutSerializer<'tmp, 'view, T, Track>
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ViewSerializer {
            view: &self.view.as_view(),
            type_names: self.type_names,
        }
        .serialize(serializer)
    }
}

impl<'view, T: Component + Serialize, Track: Tracking> Serialize for ViewMut<'view, T, Track> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ViewMutSerializer::new(self).serialize(serializer)
    }
}
