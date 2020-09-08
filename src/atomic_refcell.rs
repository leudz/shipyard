use crate::error;
use alloc::boxed::Box;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem::{ManuallyDrop, MaybeUninit};
use parking_lot::lock_api::RawRwLock as _;
#[cfg(feature = "non_sync")]
use parking_lot::lock_api::RawRwLockDowngrade as _;
use parking_lot::RawRwLock;
#[cfg(feature = "non_send")]
use std::thread::ThreadId;

/// Threadsafe `RefCell`-like container.
#[doc(hidden)]
pub struct AtomicRefCell<T: ?Sized> {
    borrow_state: RawRwLock,
    #[cfg(feature = "non_send")]
    send: Option<ThreadId>,
    #[cfg(feature = "non_sync")]
    is_sync: bool,
    _non_send_sync: PhantomData<*const ()>,
    taken: bool,
    inner: ManuallyDrop<UnsafeCell<T>>,
}

// AtomicRefCell can't be Send if it contains !Send components
#[cfg(not(feature = "non_send"))]
unsafe impl<T: ?Sized> Send for AtomicRefCell<T> {}

unsafe impl<T: ?Sized> Sync for AtomicRefCell<T> {}

impl<T: ?Sized> Drop for AtomicRefCell<T> {
    #[inline]
    fn drop(&mut self) {
        if !self.taken {
            // SAFE we're in the Drop impl so it won't be accessed again
            unsafe {
                ManuallyDrop::drop(&mut self.inner);
            }
        }
    }
}

impl<T: Send + Sync> AtomicRefCell<T> {
    /// Creates a new `AtomicRefCell` containing `value`.
    #[inline]
    pub(crate) fn new(value: T) -> Self {
        AtomicRefCell {
            borrow_state: RawRwLock::INIT,
            #[cfg(feature = "non_send")]
            send: None,
            #[cfg(feature = "non_sync")]
            is_sync: true,
            _non_send_sync: PhantomData,
            taken: false,
            inner: ManuallyDrop::new(UnsafeCell::new(value)),
        }
    }
}

#[cfg(feature = "non_send")]
impl<T: Sync> AtomicRefCell<T> {
    /// Creates a new `AtomicRefCell` containing `value`.
    #[inline]
    pub(crate) fn new_non_send(value: T, world_thread_id: ThreadId) -> Self {
        AtomicRefCell {
            borrow_state: RawRwLock::INIT,
            send: Some(world_thread_id),
            #[cfg(feature = "non_sync")]
            is_sync: true,
            _non_send_sync: PhantomData,
            taken: false,
            inner: ManuallyDrop::new(UnsafeCell::new(value)),
        }
    }
}

#[cfg(feature = "non_sync")]
impl<T: Send> AtomicRefCell<T> {
    /// Creates a new `AtomicRefCell` containing `value`.
    #[inline]
    pub(crate) fn new_non_sync(value: T) -> Self {
        AtomicRefCell {
            borrow_state: RawRwLock::INIT,
            #[cfg(feature = "non_send")]
            send: None,
            is_sync: false,
            _non_send_sync: PhantomData,
            taken: false,
            inner: ManuallyDrop::new(UnsafeCell::new(value)),
        }
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<T> AtomicRefCell<T> {
    /// Creates a new `AtomicRefCell` containing `value`.
    #[inline]
    pub(crate) fn new_non_send_sync(value: T, world_thread_id: ThreadId) -> Self {
        AtomicRefCell {
            borrow_state: RawRwLock::INIT,
            send: Some(world_thread_id),
            is_sync: false,
            _non_send_sync: PhantomData,
            taken: false,
            inner: ManuallyDrop::new(UnsafeCell::new(value)),
        }
    }
}

impl<T: ?Sized> AtomicRefCell<T> {
    /// Immutably borrows the wrapped value, returning an error if the value is currently mutably
    /// borrowed.
    ///
    /// The borrow lasts until the returned `Ref` exits scope. Multiple shared borrows can be
    /// taken out at the same time.
    #[inline]
    pub(crate) fn try_borrow(&self) -> Result<Ref<'_, T>, error::Borrow> {
        #[cfg(not(feature = "non_sync"))]
        {
            // if Send - accessible from any thread, shared xor unique
            // if !Send - accessible from any thread, shared only if not world's thread
            if self.borrow_state.try_lock_shared() {
                Ok(Ref {
                    inner: unsafe { &*self.inner.get() },
                    borrow: SharedBorrow(&self.borrow_state),
                })
            } else {
                Err(error::Borrow::Shared)
            }
        }
        #[cfg(all(not(feature = "non_send"), feature = "non_sync"))]
        {
            if self.is_sync {
                // accessible from any thread, shared xor unique
                if self.borrow_state.try_lock_shared() {
                    Ok(Ref {
                        inner: unsafe { &*self.inner.get() },
                        borrow: SharedBorrow(&self.borrow_state),
                    })
                } else {
                    Err(error::Borrow::Shared)
                }
            } else {
                // accessible from one thread at a time
                if self.borrow_state.try_lock_exclusive() {
                    unsafe {
                        self.borrow_state.downgrade();
                    }

                    Ok(Ref {
                        inner: unsafe { &*self.inner.get() },
                        borrow: SharedBorrow(&self.borrow_state),
                    })
                } else {
                    Err(error::Borrow::Shared)
                }
            }
        }
        #[cfg(all(feature = "non_send", feature = "non_sync"))]
        {
            match (self.send, self.is_sync) {
                (_, true) => {
                    // if Send - accessible from any thread, shared xor unique
                    // if !Send - accessible from any thread, shared only if not world's thread
                    if self.borrow_state.try_lock_shared() {
                        Ok(Ref {
                            inner: unsafe { &*self.inner.get() },
                            borrow: SharedBorrow(&self.borrow_state),
                        })
                    } else {
                        Err(error::Borrow::Shared)
                    }
                }
                (None, false) => {
                    // accessible from one thread at a time
                    if self.borrow_state.try_lock_exclusive() {
                        unsafe {
                            self.borrow_state.downgrade();
                        }

                        Ok(Ref {
                            inner: unsafe { &*self.inner.get() },
                            borrow: SharedBorrow(&self.borrow_state),
                        })
                    } else {
                        Err(error::Borrow::Shared)
                    }
                }
                (Some(thread_id), false) => {
                    // accessible from world's thread only
                    if thread_id != std::thread::current().id() {
                        return Err(error::Borrow::WrongThread);
                    }

                    if self.borrow_state.try_lock_shared() {
                        Ok(Ref {
                            inner: unsafe { &*self.inner.get() },
                            borrow: SharedBorrow(&self.borrow_state),
                        })
                    } else {
                        Err(error::Borrow::Shared)
                    }
                }
            }
        }
    }
    /// Mutably borrows the wrapped value, returning an error if the value is currently borrowed.
    ///
    /// The borrow lasts until the returned `RefMut` exits scope. The value cannot be borrowed while this borrow is
    /// active.
    #[inline]
    pub(crate) fn try_borrow_mut(&self) -> Result<RefMut<'_, T>, error::Borrow> {
        #[cfg(feature = "non_send")]
        {
            // if Sync - accessible from any thread, shared only if not world thread
            // if !Sync - accessible from world thread only
            if let Some(thread_id) = self.send {
                if thread_id != std::thread::current().id() {
                    return Err(error::Borrow::WrongThread);
                }
            }
        }

        if self.borrow_state.try_lock_exclusive() {
            Ok(RefMut {
                inner: unsafe { &mut *self.inner.get() },
                borrow: ExclusiveBorrow(&self.borrow_state),
            })
        } else {
            Err(error::Borrow::Unique)
        }
    }
}

impl AtomicRefCell<dyn crate::unknown_storage::UnknownStorage> {
    /// # Safety
    ///
    /// `T` has to be a unique storage of the right type.
    #[allow(clippy::boxed_local)]
    pub unsafe fn into_unique<T: 'static>(mut this: Box<Self>) -> T {
        let mut tmp: MaybeUninit<T> = MaybeUninit::uninit();

        this.taken = true;
        // SAFE both regions are valids
        tmp.as_mut_ptr()
            .copy_from_nonoverlapping((&*this.inner.get()).unique::<T>().unwrap(), 1);

        // SAFE this is initialized
        tmp.assume_init()
    }
}

pub(crate) struct SharedBorrow<'a>(&'a RawRwLock);

impl Drop for SharedBorrow<'_> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.0.unlock_shared();
        }
    }
}

impl Clone for SharedBorrow<'_> {
    #[inline]
    fn clone(&self) -> Self {
        debug_assert!(self.0.try_lock_shared());

        SharedBorrow(self.0)
    }
}

pub(crate) struct ExclusiveBorrow<'a>(&'a RawRwLock);

impl Drop for ExclusiveBorrow<'_> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.0.unlock_exclusive();
        }
    }
}

pub struct Ref<'a, T: ?Sized> {
    inner: &'a T,
    borrow: SharedBorrow<'a>,
}

impl<'a, T: ?Sized> Ref<'a, T> {
    #[inline]
    pub(crate) fn map<U, F: Fn(&'a T) -> &'a U>(this: Self, f: F) -> Ref<'a, U> {
        Ref {
            inner: f(this.inner),
            borrow: this.borrow,
        }
    }
    /// Get the inner parts of the `Ref`.
    ///
    /// # Safety
    ///
    /// The reference has to be dropped before `Borrow`.
    #[inline]
    pub(crate) unsafe fn destructure(self) -> (&'a T, SharedBorrow<'a>) {
        (self.inner, self.borrow)
    }
}

impl<T: ?Sized> core::ops::Deref for Ref<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl<T: ?Sized> Clone for Ref<'_, T> {
    #[inline]
    fn clone(&self) -> Self {
        Ref {
            inner: self.inner,
            borrow: self.borrow.clone(),
        }
    }
}

pub struct RefMut<'a, T: ?Sized> {
    inner: &'a mut T,
    borrow: ExclusiveBorrow<'a>,
}

impl<'a, T: ?Sized> RefMut<'a, T> {
    #[inline]
    pub(crate) fn map<U, F: Fn(&'a mut T) -> &'a mut U>(this: Self, f: F) -> RefMut<'a, U> {
        RefMut {
            inner: f(this.inner),
            borrow: this.borrow,
        }
    }
}

impl<T: ?Sized> core::ops::Deref for RefMut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl<T: ?Sized> core::ops::DerefMut for RefMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
    }
}

#[test]
fn shared() {
    let refcell = AtomicRefCell::new(0);
    let first_borrow = refcell.try_borrow().unwrap();

    assert!(refcell.try_borrow().is_ok());
    assert_eq!(refcell.try_borrow_mut().err(), Some(error::Borrow::Unique));

    drop(first_borrow);

    assert!(refcell.try_borrow_mut().is_ok());
}

#[test]
fn exclusive() {
    let refcell = AtomicRefCell::new(0);
    let first_borrow = refcell.try_borrow_mut().unwrap();

    assert_eq!(refcell.try_borrow().err(), Some(error::Borrow::Shared));
    assert_eq!(refcell.try_borrow_mut().err(), Some(error::Borrow::Unique));

    drop(first_borrow);

    assert!(refcell.try_borrow_mut().is_ok());
}

#[cfg(all(feature = "std", not(feature = "non_send")))]
#[test]
fn shared_thread() {
    use std::sync::Arc;

    let refcell = Arc::new(AtomicRefCell::new(0));
    let refcell_clone = refcell.clone();
    let first_borrow = refcell.try_borrow().unwrap();

    std::thread::spawn(move || {
        refcell_clone.try_borrow().unwrap();
        assert_eq!(
            refcell_clone.try_borrow_mut().err(),
            Some(error::Borrow::Unique)
        );
    })
    .join()
    .unwrap();

    drop(first_borrow);

    assert!(refcell.try_borrow_mut().is_ok());
}

#[cfg(all(feature = "std", not(feature = "non_send")))]
#[test]
fn exclusive_thread() {
    use std::sync::Arc;

    let refcell = Arc::new(AtomicRefCell::new(0));
    let refcell_clone = refcell.clone();

    std::thread::spawn(move || {
        let _first_borrow = refcell_clone.try_borrow_mut();
        assert_eq!(
            refcell_clone.try_borrow_mut().err(),
            Some(error::Borrow::Unique)
        );
    })
    .join()
    .unwrap();

    refcell.try_borrow_mut().unwrap();
}

#[cfg(feature = "non_send")]
#[test]
fn non_send() {
    let refcell = AtomicRefCell::new_non_send(0u32, std::thread::current().id());
    let refcell_ptr: *const _ = &refcell;
    let refcell_ptr = refcell_ptr as usize;

    std::thread::spawn(move || unsafe {
        (&*(refcell_ptr as *const AtomicRefCell<u32>))
            .try_borrow()
            .unwrap();
        assert_eq!(
            (&*(refcell_ptr as *const AtomicRefCell<u32>))
                .try_borrow_mut()
                .err(),
            Some(error::Borrow::WrongThread)
        );
    })
    .join()
    .unwrap();

    refcell.try_borrow().unwrap();
    refcell.try_borrow_mut().unwrap();
}

#[cfg(feature = "non_sync")]
#[test]
fn non_sync() {
    let refcell = AtomicRefCell::new_non_sync(0);

    let refcell_ptr: *const _ = &refcell;
    let refcell_ptr = refcell_ptr as usize;

    std::thread::spawn(move || unsafe {
        (&*(refcell_ptr as *const AtomicRefCell<u32>))
            .try_borrow()
            .unwrap();
        (&*(refcell_ptr as *const AtomicRefCell<u32>))
            .try_borrow_mut()
            .unwrap();
    })
    .join()
    .unwrap();

    refcell.try_borrow().unwrap();
    refcell.try_borrow_mut().unwrap();
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
#[test]
fn non_send_sync() {
    let refcell = AtomicRefCell::new_non_send_sync(0u32, std::thread::current().id());
    let refcell_ptr: *const _ = &refcell;
    let refcell_ptr = refcell_ptr as usize;

    std::thread::spawn(move || unsafe {
        assert_eq!(
            (&*(refcell_ptr as *const AtomicRefCell<u32>))
                .try_borrow()
                .err(),
            Some(error::Borrow::WrongThread)
        );
        assert_eq!(
            (&*(refcell_ptr as *const AtomicRefCell<u32>))
                .try_borrow_mut()
                .err(),
            Some(error::Borrow::WrongThread)
        );
    })
    .join()
    .unwrap();

    refcell.try_borrow().unwrap();
    refcell.try_borrow_mut().unwrap();
}
