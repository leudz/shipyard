use crate::track;

/// Indicate that a `struct` or `enum` can be store in the `World`.
pub trait Component: Sized + 'static {
    /// Specify what this storage should track.
    /// Can be one of: [`track::Untracked`], [`track::Insertion`], [`track::Modification`], [`track::Removal`], [`track::All`].
    type Tracking: track::Tracking;
}
