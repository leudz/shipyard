use core::convert::{AsMut, AsRef};
use core::ops::{Deref, DerefMut};

/// Type used to access `!Send` storages.
#[cfg_attr(docsrs, doc(cfg(feature = "thread_local")))]
pub struct NonSend<T: ?Sized>(pub(crate) T);

impl<T: ?Sized> AsRef<T> for NonSend<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized> AsMut<T> for NonSend<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: ?Sized> Deref for NonSend<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized> DerefMut for NonSend<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
