use crate::type_id::TypeId;
use crate::IntoWorkloadSystem;
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::string::String;
use core::any::Any;
use core::fmt::{Debug, Formatter};
use core::hash::{Hash, Hasher};

/// Workload identifier
pub trait Label: 'static + Send + Sync {
    #[allow(missing_docs)]
    fn as_any(&self) -> &dyn Any;
    #[allow(missing_docs)]
    fn dyn_eq(&self, other: &dyn Label) -> bool;
    #[allow(missing_docs)]
    fn dyn_hash(&self, state: &mut dyn Hasher);
    #[allow(missing_docs)]
    fn dyn_clone(&self) -> Box<dyn Label>;
    #[allow(missing_docs)]
    fn dyn_debug(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error>;
}

macro_rules! impl_label {
    ($($type: ty),+) => {
        $(
            impl Label for $type {
                fn as_any(&self) -> &dyn Any {
                    self
                }
                fn dyn_eq(&self, other: &dyn Label) -> bool {
                    if let Some(other) = other.as_any().downcast_ref::<Self>() {
                        self == other
                    } else {
                        false
                    }
                }
                fn dyn_hash(&self, mut state: &mut dyn Hasher) {
                    Self::hash(self, &mut state);
                }
                fn dyn_clone(&self) -> Box<dyn Label> {
                    Box::new(self.clone())
                }
                fn dyn_debug(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
                    Self::fmt(self, f)
                }
            }
        )+
    };
}

impl_label![&'static str, String, Cow<'static, str>, TypeId];

impl Label for Box<dyn Label> {
    fn as_any(&self) -> &dyn Any {
        (**self).as_any()
    }
    fn dyn_eq(&self, other: &dyn Label) -> bool {
        (**self).dyn_eq(other)
    }
    fn dyn_hash(&self, state: &mut dyn Hasher) {
        (**self).dyn_hash(state);
    }
    fn dyn_clone(&self) -> Box<dyn Label> {
        (**self).dyn_clone()
    }
    fn dyn_debug(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        (**self).dyn_debug(f)
    }
}

impl Label for crate::Workload {
    fn as_any(&self) -> &dyn Any {
        &self.name
    }
    fn dyn_eq(&self, other: &dyn Label) -> bool {
        self.name.dyn_eq(other)
    }
    fn dyn_hash(&self, state: &mut dyn Hasher) {
        self.name.dyn_hash(state)
    }
    fn dyn_clone(&self) -> Box<dyn Label> {
        self.name.dyn_clone()
    }
    fn dyn_debug(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        self.name.dyn_debug(f)
    }
}

impl Clone for Box<dyn Label> {
    fn clone(&self) -> Self {
        // Box<dyn Label> implements Label, we have to deref to get the actual type
        (**self).dyn_clone()
    }
}

impl Hash for dyn Label {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_any().type_id().hash(state);
        self.dyn_hash(state);
    }
}

impl PartialEq for dyn Label {
    fn eq(&self, other: &Self) -> bool {
        self.dyn_eq(other)
    }
}

impl Eq for dyn Label {}

impl Debug for dyn Label {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        self.dyn_debug(f)
    }
}

/// Converts a value to a `Box<dyn Label>`
pub trait AsLabel<T> {
    #[allow(missing_docs)]
    fn as_label(&self) -> Box<dyn Label>;
}

impl<Views, R, W> AsLabel<(Views, R)> for W
where
    W: IntoWorkloadSystem<Views, R> + 'static,
{
    fn as_label(&self) -> Box<dyn Label> {
        Box::new(TypeId::of::<W>())
    }
}

impl<T: Label> AsLabel<T> for T {
    fn as_label(&self) -> Box<dyn Label> {
        T::dyn_clone(self)
    }
}

#[derive(Clone, Debug, Hash)]
pub(crate) struct SequentialLabel(pub(crate) Box<dyn Label>);

impl Label for SequentialLabel {
    fn as_any(&self) -> &dyn Any {
        self
    }

    #[allow(clippy::op_ref)]
    fn dyn_eq(&self, other: &dyn Label) -> bool {
        if let Some(other) = other.as_any().downcast_ref::<SequentialLabel>() {
            &self.0 == &other.0
        } else {
            false
        }
    }

    fn dyn_hash(&self, mut state: &mut dyn Hasher) {
        SequentialLabel::hash(self, &mut state)
    }

    fn dyn_clone(&self) -> Box<dyn Label> {
        Box::new(SequentialLabel(self.0.clone()))
    }

    fn dyn_debug(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        SequentialLabel::fmt(self, f)
    }
}
