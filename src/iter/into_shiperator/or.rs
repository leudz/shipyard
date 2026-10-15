use crate::iter::IntoShiperator;
use crate::or::{Or, OrWindow};
use crate::sparse_set::RawEntityIdAccess;

impl<T: IntoShiperator, U: IntoShiperator> IntoShiperator for Or<(T, U)> {
    type Shiperator = OrWindow<(T::Shiperator, U::Shiperator)>;

    #[inline]
    fn into_shiperator(self) -> (Self::Shiperator, usize, RawEntityIdAccess) {
        let (shiperator1, len1, mut entity_access) = (self.0).0.into_shiperator();
        let (shiperator2, _, entity_access2) = (self.0).1.into_shiperator();

        // The second storage's entities are iterated once the first's are exhausted.
        entity_access.push_follow_up(entity_access2.cursor());

        (
            OrWindow {
                storages: (shiperator1, shiperator2),
                is_past_first_storage: false,
            },
            len1,
            entity_access,
        )
    }

    #[inline]
    fn can_captain() -> bool {
        true
    }

    #[inline]
    fn can_sailor() -> bool {
        true
    }
}
