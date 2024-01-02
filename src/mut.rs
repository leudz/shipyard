use crate::tracking::TrackingTimestamp;

/// Tracks component modification.
pub struct Mut<'a, T: ?Sized> {
    pub(crate) flag: Option<&'a mut TrackingTimestamp>,
    pub(crate) current: TrackingTimestamp,
    pub(crate) data: &'a mut T,
}

impl<'a, T: ?Sized> Mut<'a, T> {
    /// Makes a new [`Mut`], the component will not be flagged if its modified inside `f`.
    ///
    /// This is an associated function that needs to be used as `Mut::map(...)`. A method would interfere with methods of the same name used through Deref.
    pub fn map<U: ?Sized, F: FnOnce(&mut T) -> &mut U>(orig: Self, f: F) -> Mut<'a, U> {
        Mut {
            flag: orig.flag,
            current: orig.current,
            data: f(orig.data),
        }
    }
}

impl<T: ?Sized> core::ops::Deref for Mut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T: ?Sized> core::ops::DerefMut for Mut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        if let Some(flag) = &mut self.flag {
            **flag = self.current;
        }

        self.data
    }
}

impl<T: ?Sized> AsRef<T> for Mut<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        self.data
    }
}

impl<T: ?Sized> AsMut<T> for Mut<'_, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        if let Some(flag) = &mut self.flag {
            **flag = self.current;
        }

        self.data
    }
}

impl<T: ?Sized + core::fmt::Debug> core::fmt::Debug for Mut<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.data.fmt(f)
    }
}
