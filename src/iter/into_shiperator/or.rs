use crate::iter::IntoShiperator;
use crate::or::{Or, OrWindow};
use crate::sparse_set::RawEntityIdAccess;
use crate::storage::StorageId;
use crate::ShipHashSet;
use alloc::vec;

impl<T: IntoShiperator, U: IntoShiperator> IntoShiperator for Or<(T, U)> {
    type Shiperator = OrWindow<(T::Shiperator, U::Shiperator)>;

    #[inline]
    fn into_shiperator(
        self,
        storage_ids: &mut ShipHashSet<StorageId>,
    ) -> (Self::Shiperator, usize, RawEntityIdAccess) {
        let (shiperator1, len1, entity_access1) = (self.0).0.into_shiperator(storage_ids);
        let (shiperator2, len2, entity_access2) = (self.0).1.into_shiperator(storage_ids);

        let entity_access =
            RawEntityIdAccess::new(entity_access1.ptr, vec![(entity_access2.ptr, len2)]);

        (
            OrWindow {
                storages: (shiperator1, shiperator2),
                is_captain: true,
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
