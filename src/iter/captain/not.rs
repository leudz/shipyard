use crate::component::Component;
use crate::iter::ShiperatorCaptain;
use crate::not::Not;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::tracking::{Inserted, InsertedOrModified, Modified};

const NOT_FACTOR: f32 = 1.2;

impl<'tmp, T: Component> ShiperatorCaptain for Not<FullRawWindow<'tmp, T>> {
    #[inline]
    unsafe fn get_captain_data(&self, _index: usize) -> Self::Out {
        unreachable!()
    }

    #[inline]
    fn next_slice(&mut self) {}

    #[inline]
    #[allow(clippy::cast_precision_loss)]
    fn sail_time(&self) -> usize {
        (self.0.sail_time() as f32 * NOT_FACTOR) as usize
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

impl<'tmp, T: Component, Track> ShiperatorCaptain for Not<FullRawWindowMut<'tmp, T, Track>> {
    #[inline]
    unsafe fn get_captain_data(&self, _index: usize) -> Self::Out {
        unreachable!()
    }

    #[inline]
    fn next_slice(&mut self) {}

    #[inline]
    fn sail_time(&self) -> usize {
        usize::MAX
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        false
    }

    #[inline]
    fn unpick(&mut self) {}
}

macro_rules! impl_shiperator_captain_not_tracking {
    ($($type: ident)+) => {$(
        impl<'tmp, T: ShiperatorCaptain> ShiperatorCaptain for Not<$type<T>> {
            #[inline]
            unsafe fn get_captain_data(&self, _index: usize) -> Self::Out {
                unreachable!()
            }

            #[inline]
            fn next_slice(&mut self) {}

            #[inline]
            #[allow(clippy::cast_precision_loss)]
            fn sail_time(&self) -> usize {
                (self.0.sail_time() as f32 * NOT_FACTOR) as usize
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

impl_shiperator_captain_not_tracking![Inserted Modified InsertedOrModified];
