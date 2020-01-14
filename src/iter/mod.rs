mod abstract_mut;
mod enumerate;
mod filter;
mod into_abstract;
mod into_iter;
pub mod iterators;
mod map;
#[cfg(feature = "parallel")]
mod parallel_buffer;
mod shiperator;
mod with_id;

pub use enumerate::Enumerate;
pub use filter::Filter;
pub use into_iter::{IntoIter, IntoIterIds};
pub use iterators::*;
pub use map::Map;
pub use shiperator::{CurrentId, Shiperator};
pub use with_id::WithId;

impl<T> IntoIterIds for T
where
    T: IntoIter,
    <T as IntoIter>::IntoIter: CurrentId,
{
    #[allow(clippy::type_complexity)]
    type IntoIterIds = Map<
        WithId<<T as IntoIter>::IntoIter>,
        fn(
            (
                <<T as IntoIter>::IntoIter as CurrentId>::Id,
                <<T as IntoIter>::IntoIter as Shiperator>::Item,
            ),
        ) -> <<T as IntoIter>::IntoIter as CurrentId>::Id,
    >;

    fn iter_ids(self) -> Self::IntoIterIds {
        self.iter().with_id().map(|(id, _)| id)
    }
}
