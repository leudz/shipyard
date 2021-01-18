use core::convert::{AsMut, AsRef};
use core::ops::{Deref, DerefMut};

/// Type used to access `!Sync` storages.
#[cfg_attr(docsrs, doc(cfg(feature = "thread_local")))]
pub struct NonSync<T: ?Sized>(pub(crate) T);

impl<T: ?Sized> AsRef<T> for NonSync<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized> AsMut<T> for NonSync<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: ?Sized> Deref for NonSync<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized> DerefMut for NonSync<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
