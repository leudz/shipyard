use crate::component::Component;
use crate::iter::planner::ConstructionMetadata;
use crate::iter::{IntoShiperator, RawEntityIdAccess};
use crate::not::Not;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::tracking::{
    Inserted, InsertedOrModified, InsertionOrModificationTracking, InsertionTracking,
    ModificationTracking, Modified, Tracking,
};
use crate::views::{View, ViewMut};

// SAFE: We wrap the view's own results. `Not` can't captain so it's only ever looked up,
// its entities are marked filtered and never drive a grouped walk. It yields `()`.
unsafe impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
    for Not<&'tmp View<'v, T, Track>>
{
    type Shiperator = Not<FullRawWindow<'tmp, T, Track>>;

    #[inline]
    fn into_shiperator(self) -> (Self::Shiperator, RawEntityIdAccess, ConstructionMetadata) {
        let (window, entities, _) = self.0.into_shiperator();

        (Not(window), entities, ConstructionMetadata::filtered())
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

// SAFE: See `Not<&View>`.
unsafe impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
    for Not<&'tmp ViewMut<'v, T, Track>>
{
    type Shiperator = Not<FullRawWindow<'tmp, T, Track>>;

    #[inline]
    fn into_shiperator(self) -> (Self::Shiperator, RawEntityIdAccess, ConstructionMetadata) {
        let (window, entities, _) = self.0.into_shiperator();

        (Not(window), entities, ConstructionMetadata::filtered())
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

// SAFE: See `Not<&View>`.
unsafe impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
    for Not<&'tmp mut ViewMut<'v, T, Track>>
{
    type Shiperator = Not<FullRawWindowMut<'tmp, T, Track>>;

    #[inline]
    fn into_shiperator(self) -> (Self::Shiperator, RawEntityIdAccess, ConstructionMetadata) {
        let (window, entities, _) = self.0.into_shiperator();

        (Not(window), entities, ConstructionMetadata::filtered())
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
    ($($type: ident => $capability: ident)+) => {$(
        // SAFE: We wrap the view's own results and mark them filtered, they never drive a grouped
        // walk. The filter only rejects candidates, it never yields a component.
        unsafe impl<'tmp, 'v: 'tmp, T: Component, Track: $capability> IntoShiperator
            for Not<$type<&'tmp View<'v, T, Track>>>
        {
            type Shiperator = Not<$type<FullRawWindow<'tmp, T, Track>>>;

            #[inline]
            fn into_shiperator(
                self,
            ) -> (Self::Shiperator, RawEntityIdAccess, ConstructionMetadata) {
                let (window, entities, _) = (self.0).0.into_shiperator();

                (Not($type(window)), entities, ConstructionMetadata::filtered())
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
        // walk. The filter only rejects candidates, it never yields a component.
        unsafe impl<'tmp, 'v: 'tmp, T: Component, Track: $capability> IntoShiperator
            for Not<$type<&'tmp ViewMut<'v, T, Track>>>
        {
            type Shiperator = Not<$type<FullRawWindow<'tmp, T, Track>>>;

            #[inline]
            fn into_shiperator(
                self,
            ) -> (Self::Shiperator, RawEntityIdAccess, ConstructionMetadata) {
                let (window, entities, _) = (self.0).0.into_shiperator();

                (Not($type(window)), entities, ConstructionMetadata::filtered())
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
        // walk. The filter only rejects candidates, it never yields a component.
        unsafe impl<'tmp, 'v: 'tmp, T: Component, Track: $capability> IntoShiperator
            for Not<$type<&'tmp mut ViewMut<'v, T, Track>>>
        {
            type Shiperator = Not<$type<FullRawWindowMut<'tmp, T, Track>>>;

            #[inline]
            fn into_shiperator(
                self,
            ) -> (Self::Shiperator, RawEntityIdAccess, ConstructionMetadata) {
                let (window, entities, _) = (self.0).0.into_shiperator();

                (Not($type(window)), entities, ConstructionMetadata::filtered())
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
