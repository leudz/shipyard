use crate::component::Component;
use crate::iter::IntoShiperator;
use crate::not::Not;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut, RawEntityIdAccess};
use crate::storage::StorageId;
use crate::tracking::{Inserted, InsertedOrModified, Modified, Tracking};
use crate::views::{View, ViewMut};
use crate::ShipHashSet;

impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
    for Not<&'tmp View<'v, T, Track>>
{
    type Shiperator = Not<FullRawWindow<'tmp, T>>;

    #[inline]
    fn into_shiperator(
        self,
        storage_ids: &mut ShipHashSet<StorageId>,
    ) -> (Self::Shiperator, usize, RawEntityIdAccess) {
        let (window, len, entity_access) = self.0.into_shiperator(storage_ids);

        (Not(window), len, entity_access)
    }

    #[inline]
    fn can_captain() -> bool {
        false
    }

    #[inline]
    fn can_sailor() -> bool {
        true
    }
}

impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
    for Not<&'tmp ViewMut<'v, T, Track>>
{
    type Shiperator = Not<FullRawWindow<'tmp, T>>;

    #[inline]
    fn into_shiperator(
        self,
        storage_ids: &mut ShipHashSet<StorageId>,
    ) -> (Self::Shiperator, usize, RawEntityIdAccess) {
        let (window, len, entity_access) = self.0.into_shiperator(storage_ids);

        (Not(window), len, entity_access)
    }

    #[inline]
    fn can_captain() -> bool {
        false
    }

    #[inline]
    fn can_sailor() -> bool {
        true
    }
}

impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
    for Not<&'tmp mut ViewMut<'v, T, Track>>
{
    type Shiperator = Not<FullRawWindowMut<'tmp, T, Track>>;

    #[inline]
    fn into_shiperator(
        self,
        storage_ids: &mut ShipHashSet<StorageId>,
    ) -> (Self::Shiperator, usize, RawEntityIdAccess) {
        let (window, len, entity_access) = self.0.into_shiperator(storage_ids);

        (Not(window), len, entity_access)
    }

    #[inline]
    fn can_captain() -> bool {
        false
    }

    #[inline]
    fn can_sailor() -> bool {
        true
    }
}

macro_rules! impl_into_shiperator_tracking {
    ($($type: ident)+) => {$(
        impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
            for Not<$type<&'tmp View<'v, T, Track>>>
        {
            type Shiperator = Not<$type<FullRawWindow<'tmp, T>>>;

            #[inline]
            fn into_shiperator(
                self,
                storage_ids: &mut ShipHashSet<StorageId>,
            ) -> (Self::Shiperator, usize, RawEntityIdAccess) {
                let (window, len, entity_access) = (self.0).0.into_shiperator(storage_ids);

                (Not($type(window)), len, entity_access)
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

        impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
            for Not<$type<&'tmp ViewMut<'v, T, Track>>>
        {
            type Shiperator = Not<$type<FullRawWindow<'tmp, T>>>;

            #[inline]
            fn into_shiperator(
                self,
                storage_ids: &mut ShipHashSet<StorageId>,
            ) -> (Self::Shiperator, usize, RawEntityIdAccess) {
                let (window, len, entity_access) = (self.0).0.into_shiperator(storage_ids);

                (Not($type(window)), len, entity_access)
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

        impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
            for Not<$type<&'tmp mut ViewMut<'v, T, Track>>>
        {
            type Shiperator = Not<$type<FullRawWindowMut<'tmp, T, Track>>>;

            #[inline]
            fn into_shiperator(
                self,
                storage_ids: &mut ShipHashSet<StorageId>,
            ) -> (Self::Shiperator, usize, RawEntityIdAccess) {
                let (window, len, entity_access) = (self.0).0.into_shiperator(storage_ids);

                (Not($type(window)), len, entity_access)
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
    )+};
}

impl_into_shiperator_tracking![Inserted Modified InsertedOrModified];
