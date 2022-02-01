use alloc::boxed::Box;
use core::any::Any;
use core::fmt::{Debug, Formatter};
use core::hash::{Hash, Hasher};

/// Workload identifier
///
/// Implemented for all types `'static + Send + Sync + Clone + Hash + Eq + Debug`
pub trait Label: 'static + Send + Sync {
    #[allow(missing_docs)]
    fn as_any(&self) -> &dyn Any;
    #[allow(missing_docs)]
    fn dyn_eq(&self, other: &dyn Label) -> bool;
    #[allow(missing_docs)]
    fn dyn_hash(&self, hasher: &mut dyn Hasher);
    #[allow(missing_docs)]
    fn dyn_clone(&self) -> Box<dyn Label>;
    #[allow(missing_docs)]
    fn dyn_debug(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error>;
}

impl<L: 'static + Send + Sync + Clone + Hash + Eq + Debug> Label for L {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn dyn_eq(&self, other: &dyn Label) -> bool {
        if let Some(other) = other.as_any().downcast_ref::<L>() {
            self == other
        } else {
            false
        }
    }

    fn dyn_hash(&self, mut state: &mut dyn Hasher) {
        L::hash(self, &mut state);
    }

    fn dyn_clone(&self) -> Box<dyn Label> {
        Box::new(L::clone(self))
    }

    fn dyn_debug(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        L::fmt(self, f)
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
