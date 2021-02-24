use crate::entity_id::EntityId;

/// Tracks component modification.
pub struct Mut<'a, T> {
    pub(crate) flag: Option<&'a mut EntityId>,
    pub(crate) data: &'a mut T,
}

impl<T> core::ops::Deref for Mut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T> core::ops::DerefMut for Mut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        if let Some(flag) = &mut self.flag {
            flag.set_modified();
        }

        self.data
    }
}

impl<T> AsRef<T> for Mut<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        self.data
    }
}

impl<T> AsMut<T> for Mut<'_, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        if let Some(flag) = &mut self.flag {
            flag.set_modified();
        }

        self.data
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for Mut<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.data.fmt(f)
    }
}
