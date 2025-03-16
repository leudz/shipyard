use crate::component::Component;
use crate::iter::IntoShiperator;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut, RawEntityIdAccess};
use crate::storage::StorageId;
use crate::tracking::{Inserted, InsertedOrModified, Modified, Tracking};
use crate::views::{View, ViewMut};
use crate::ShipHashSet;

macro_rules! impl_into_shiperator_tracking {
    ($($type: ident)+) => {$(
        impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
            for $type<&'tmp View<'v, T, Track>>
        {
            type Shiperator = $type<FullRawWindow<'tmp, T>>;

            #[inline]
            fn into_shiperator(
                self,
                storage_ids: &mut ShipHashSet<StorageId>,
            ) -> (Self::Shiperator, usize, RawEntityIdAccess) {
                let (window, len, entity_access) = self.0.into_shiperator(storage_ids);

                ($type(window), len, entity_access)
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
            for $type<&'tmp ViewMut<'v, T, Track>>
        {
            type Shiperator = $type<FullRawWindow<'tmp, T>>;

            #[inline]
            fn into_shiperator(
                self,
                storage_ids: &mut ShipHashSet<StorageId>,
            ) -> (Self::Shiperator, usize, RawEntityIdAccess) {
                let (window, len, entity_access) = self.0.into_shiperator(storage_ids);

                ($type(window), len, entity_access)
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
            for $type<&'tmp mut ViewMut<'v, T, Track>>
        {
            type Shiperator = $type<FullRawWindowMut<'tmp, T, Track>>;

            #[inline]
            fn into_shiperator(
                self,
                storage_ids: &mut ShipHashSet<StorageId>,
            ) -> (Self::Shiperator, usize, RawEntityIdAccess) {
                let (window, len, entity_access) = self.0.into_shiperator(storage_ids);

                ($type(window), len, entity_access)
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
