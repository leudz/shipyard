use crate::component::Component;
use crate::iter::IntoShiperator;
use crate::iter::RawEntityIdAccess;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::tracking::{
    Inserted, InsertedOrModified, InsertionOrModificationTracking, InsertionTracking,
    ModificationTracking, Modified,
};
use crate::views::{View, ViewMut};

macro_rules! impl_into_shiperator_tracking {
    ($($type: ident => $capability: ident)+) => {$(
        // SAFE: We wrap the view's own results and mark them filtered, they never drive a grouped
        // walk. The filter only rejects candidates of the view's window, which upholds the rest.
        unsafe impl<'tmp, 'v: 'tmp, T: Component, Track: $capability> IntoShiperator
            for $type<&'tmp View<'v, T, Track>>
        {
            type Shiperator = $type<FullRawWindow<'tmp, T, Track>>;

            #[inline]
            fn into_shiperator(
                self,
            ) -> (Self::Shiperator, usize, RawEntityIdAccess) {
                let (window, len, entity_access) = self.0.into_shiperator();

                ($type(window), len, entity_access.filtered())
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

        // SAFE: We wrap the view's own results and mark them filtered, they never drive a grouped
        // walk. The filter only rejects candidates of the view's window, which upholds the rest.
        unsafe impl<'tmp, 'v: 'tmp, T: Component, Track: $capability> IntoShiperator
            for $type<&'tmp ViewMut<'v, T, Track>>
        {
            type Shiperator = $type<FullRawWindow<'tmp, T, Track>>;

            #[inline]
            fn into_shiperator(
                self,
            ) -> (Self::Shiperator, usize, RawEntityIdAccess) {
                let (window, len, entity_access) = self.0.into_shiperator();

                ($type(window), len, entity_access.filtered())
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

        // SAFE: We wrap the view's own results and mark them filtered, they never drive a grouped
        // walk. The filter only rejects candidates of the view's window, which upholds the rest.
        unsafe impl<'tmp, 'v: 'tmp, T: Component, Track: $capability> IntoShiperator
            for $type<&'tmp mut ViewMut<'v, T, Track>>
        {
            type Shiperator = $type<FullRawWindowMut<'tmp, T, Track>>;

            #[inline]
            fn into_shiperator(
                self,
            ) -> (Self::Shiperator, usize, RawEntityIdAccess) {
                let (window, len, entity_access) = self.0.into_shiperator();

                ($type(window), len, entity_access.filtered())
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

impl_into_shiperator_tracking![
    Inserted => InsertionTracking
    Modified => ModificationTracking
    InsertedOrModified => InsertionOrModificationTracking
];
