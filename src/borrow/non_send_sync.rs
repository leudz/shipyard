use core::convert::{AsMut, AsRef};
use core::ops::{Deref, DerefMut};

/// Type used to access `!Send + !Sync` storages.
#[cfg_attr(docsrs, doc(cfg(all(feature = "non_send", feature = "non_sync"))))]
pub struct NonSendSync<T: ?Sized>(pub(crate) T);

impl<T: ?Sized> AsRef<T> for NonSendSync<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized> AsMut<T> for NonSendSync<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: ?Sized> Deref for NonSendSync<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized> DerefMut for NonSendSync<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
