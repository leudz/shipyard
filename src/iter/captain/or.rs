use crate::iter::ShiperatorCaptain;
use crate::or::OrWindow;

impl<T: ShiperatorCaptain, U: ShiperatorCaptain> ShiperatorCaptain for OrWindow<(T, U)> {
    #[inline]
    unsafe fn get_captain_data(&self, _index: usize) -> Self::Out {
        unreachable!()
    }

    #[inline]
    fn next_slice(&mut self) {
        self.is_past_first_storage = true;
    }

    #[inline]
    fn sail_time(&self) -> usize {
        (self.storages).0.sail_time() + (self.storages).1.sail_time()
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        false
    }

    #[inline]
    fn unpick(&mut self) {
        (self.storages).0.unpick();
        (self.storages).1.unpick();
        self.is_captain = false;
    }
}
