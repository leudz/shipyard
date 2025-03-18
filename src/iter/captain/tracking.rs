use crate::iter::ShiperatorCaptain;
use crate::tracking::{Inserted, InsertedOrModified, Modified};

const TRACKING_FACTOR: f32 = 2.0;

macro_rules! impl_shiperator_captain_tracking {
    ($($type: ident)+) => {$(
        impl<'tmp, T: ShiperatorCaptain> ShiperatorCaptain for $type<T> {
            #[inline]
            unsafe fn get_captain_data(&self, _index: usize) -> Self::Out {
                unreachable!()
            }

            #[inline]
            fn next_slice(&mut self) {}

            #[inline]
            #[allow(clippy::cast_precision_loss)]
            fn sail_time(&self) -> usize {
                (self.0.sail_time() as f32 * TRACKING_FACTOR) as usize
            }

            #[inline]
            fn is_exact_sized(&self) -> bool {
                false
            }

            #[inline]
            fn unpick(&mut self) {
                self.0.unpick();
            }
        }
    )+};
}

impl_shiperator_captain_tracking![Inserted Modified InsertedOrModified];
