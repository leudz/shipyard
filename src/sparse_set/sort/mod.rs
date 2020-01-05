mod unstable;

pub use unstable::*;

pub trait IntoSortable {
    type IntoSortable;

    fn sort(self) -> Self::IntoSortable;
}
