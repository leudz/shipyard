mod unstable;

use super::SparseSet;
pub use unstable::*;

/// Trait used to sort storage(s).
pub trait IntoSortable {
    type IntoSortable;

    fn sort(self) -> Self::IntoSortable;
}
