use alloc::boxed::Box;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};

pub(crate) enum RwLock<T> {
    Custom {
        lock: Box<dyn ShipyardRwLock + Send + Sync>,
        value: UnsafeCell<T>,
    },
    #[cfg(feature = "std")]
    Std { lock: std::sync::RwLock<T> },
}

pub(crate) enum ReadGuard<'a, T> {
    Custom {
        lock: &'a dyn ShipyardRwLock,
        value: &'a T,
        marker: core::marker::PhantomData<(&'a T, lock_api::GuardNoSend)>,
    },
    #[cfg(feature = "std")]
    Std {
        guard: std::sync::RwLockReadGuard<'a, T>,
    },
}

impl<'a, T> Deref for ReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Custom { value, .. } => value,
            #[cfg(feature = "std")]
            Self::Std { guard } => guard,
        }
    }
}

impl<T> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        match self {
            Self::Custom { lock, .. } => unsafe {
                lock.unlock_shared();
            },
            #[cfg(feature = "std")]
            Self::Std { .. } => {}
        }
    }
}

pub(crate) enum WriteGuard<'a, T> {
    Custom {
        lock: &'a dyn ShipyardRwLock,
        value: &'a mut T,
        marker: core::marker::PhantomData<(&'a mut T, lock_api::GuardNoSend)>,
    },
    #[cfg(feature = "std")]
    Std {
        guard: std::sync::RwLockWriteGuard<'a, T>,
    },
}

impl<'a, T> Deref for WriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Custom { value, .. } => value,
            #[cfg(feature = "std")]
            Self::Std { guard } => guard,
        }
    }
}

impl<'a, T> DerefMut for WriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Custom { value, .. } => value,
            #[cfg(feature = "std")]
            Self::Std { guard } => guard,
        }
    }
}

impl<T> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        match self {
            Self::Custom { lock, .. } => unsafe {
                lock.unlock_exclusive();
            },
            #[cfg(feature = "std")]
            Self::Std { .. } => {}
        }
    }
}

impl<T> RwLock<T> {
    pub(crate) fn new_custom(lock: Box<dyn ShipyardRwLock + Send + Sync>, value: T) -> Self {
        RwLock::Custom {
            lock,
            value: UnsafeCell::new(value),
        }
    }
    #[cfg(feature = "std")]
    pub(crate) fn new_std(value: T) -> Self {
        RwLock::Std {
            lock: std::sync::RwLock::new(value),
        }
    }
    pub(crate) fn read(&self) -> ReadGuard<'_, T> {
        match self {
            RwLock::Custom { lock, value } => {
                lock.lock_shared();

                ReadGuard::Custom {
                    lock: &**lock,
                    value: unsafe { &*value.get() },
                    marker: core::marker::PhantomData,
                }
            }
            #[cfg(feature = "std")]
            RwLock::Std { lock } => ReadGuard::Std {
                guard: lock.read().unwrap(),
            },
        }
    }
    pub(crate) fn write(&self) -> WriteGuard<'_, T> {
        match self {
            RwLock::Custom { lock, value } => {
                lock.lock_exclusive();

                WriteGuard::Custom {
                    lock: &**lock,
                    value: unsafe { &mut *value.get() },
                    marker: core::marker::PhantomData,
                }
            }
            #[cfg(feature = "std")]
            RwLock::Std { lock } => WriteGuard::Std {
                guard: lock.write().unwrap(),
            },
        }
    }
    pub(crate) fn get_mut(&mut self) -> &mut T {
        match self {
            RwLock::Custom { value, .. } => value.get_mut(),
            #[cfg(feature = "std")]
            RwLock::Std { lock } => lock.get_mut().unwrap(),
        }
    }
}

pub trait ShipyardRwLock {
    #[allow(clippy::new_ret_no_self)]
    fn new() -> Box<dyn ShipyardRwLock + Send + Sync>
    where
        Self: Sized;
    fn lock_shared(&self);
    unsafe fn unlock_shared(&self);
    fn lock_exclusive(&self);
    unsafe fn unlock_exclusive(&self);
}

impl<T> ShipyardRwLock for T
where
    T: 'static + lock_api::RawRwLock + Send + Sync,
{
    fn new() -> Box<dyn ShipyardRwLock + Send + Sync>
    where
        Self: Sized,
    {
        Box::new(T::INIT)
    }
    fn lock_shared(&self) {
        lock_api::RawRwLock::lock_shared(self)
    }

    unsafe fn unlock_shared(&self) {
        lock_api::RawRwLock::unlock_shared(self)
    }

    fn lock_exclusive(&self) {
        lock_api::RawRwLock::lock_exclusive(self)
    }

    unsafe fn unlock_exclusive(&self) {
        lock_api::RawRwLock::unlock_exclusive(self)
    }
}
