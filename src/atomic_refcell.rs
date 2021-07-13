mod borrow_state;

pub use borrow_state::{ExclusiveBorrow, SharedBorrow};

use crate::error;
use borrow_state::BorrowState;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
#[cfg(feature = "thread_local")]
use std::thread::ThreadId;

/// Threadsafe `RefCell`-like container.
#[doc(hidden)]
pub struct AtomicRefCell<T: ?Sized> {
    borrow_state: BorrowState,
    #[cfg(feature = "thread_local")]
    send: Option<ThreadId>,
    #[cfg(feature = "thread_local")]
    is_sync: bool,
    _non_send_sync: PhantomData<*const ()>,
    inner: UnsafeCell<T>,
}

// AtomicRefCell can't be Send if it contains !Send components
#[cfg(not(feature = "thread_local"))]
unsafe impl<T: ?Sized> Send for AtomicRefCell<T> {}

unsafe impl<T: ?Sized> Sync for AtomicRefCell<T> {}

impl<T: Send + Sync> AtomicRefCell<T> {
    /// Creates a new `AtomicRefCell` containing `value`.
    #[inline]
    pub(crate) fn new(value: T) -> Self {
        AtomicRefCell {
            borrow_state: BorrowState::new(),
            #[cfg(feature = "thread_local")]
            send: None,
            #[cfg(feature = "thread_local")]
            is_sync: true,
            _non_send_sync: PhantomData,
            inner: UnsafeCell::new(value),
        }
    }
}

#[cfg(feature = "thread_local")]
impl<T: Sync> AtomicRefCell<T> {
    /// Creates a new `AtomicRefCell` containing `value`.
    #[inline]
    pub(crate) fn new_non_send(value: T, world_thread_id: ThreadId) -> Self {
        AtomicRefCell {
            borrow_state: BorrowState::new(),
            send: Some(world_thread_id),
            #[cfg(feature = "thread_local")]
            is_sync: true,
            _non_send_sync: PhantomData,
            inner: UnsafeCell::new(value),
        }
    }
}

#[cfg(feature = "thread_local")]
impl<T: Send> AtomicRefCell<T> {
    /// Creates a new `AtomicRefCell` containing `value`.
    #[inline]
    pub(crate) fn new_non_sync(value: T) -> Self {
        AtomicRefCell {
            borrow_state: BorrowState::new(),
            #[cfg(feature = "thread_local")]
            send: None,
            is_sync: false,
            _non_send_sync: PhantomData,
            inner: UnsafeCell::new(value),
        }
    }
}

#[cfg(feature = "thread_local")]
impl<T> AtomicRefCell<T> {
    /// Creates a new `AtomicRefCell` containing `value`.
    #[inline]
    pub(crate) fn new_non_send_sync(value: T, world_thread_id: ThreadId) -> Self {
        AtomicRefCell {
            borrow_state: BorrowState::new(),
            send: Some(world_thread_id),
            is_sync: false,
            _non_send_sync: PhantomData,
            inner: UnsafeCell::new(value),
        }
    }
}

impl<T> AtomicRefCell<T> {
    #[inline]
    pub(crate) fn into_inner(self) -> T {
        self.inner.into_inner()
    }
}

impl<T: ?Sized> AtomicRefCell<T> {
    /// Immutably borrows the wrapped value, returning an error if the value is currently mutably
    /// borrowed.
    ///
    /// The borrow lasts until the returned `Ref` exits scope. Multiple shared borrows can be
    /// taken out at the same time.
    #[inline]
    pub(crate) fn borrow(&self) -> Result<Ref<'_, &'_ T>, error::Borrow> {
        #[cfg(not(feature = "thread_local"))]
        {
            // if Send - accessible from any thread, shared xor unique
            // if !Send - accessible from any thread, shared only if not world's thread
            match self.borrow_state.read() {
                Ok(borrow) => Ok(Ref {
                    inner: unsafe { &*self.inner.get() },
                    borrow,
                }),
                Err(err) => Err(err),
            }
        }
        #[cfg(feature = "thread_local")]
        {
            match (self.send, self.is_sync) {
                (_, true) => {
                    // if Send - accessible from any thread, shared xor unique
                    // if !Send - accessible from any thread, shared only if not world's thread
                    match self.borrow_state.read() {
                        Ok(borrow) => Ok(Ref {
                            inner: unsafe { &*self.inner.get() },
                            borrow,
                        }),
                        Err(err) => Err(err),
                    }
                }
                (None, false) => {
                    // accessible from one thread at a time
                    match self.borrow_state.exclusive_read() {
                        Ok(borrow) => {
                            Ok(Ref {
                                // SAFE we locked
                                inner: unsafe { &*self.inner.get() },
                                borrow,
                            })
                        }
                        Err(err) => Err(err),
                    }
                }
                (Some(thread_id), false) => {
                    // accessible from world's thread only
                    if thread_id != std::thread::current().id() {
                        return Err(error::Borrow::WrongThread);
                    }

                    match self.borrow_state.read() {
                        Ok(borrow) => {
                            Ok(Ref {
                                // SAFE we locked
                                inner: unsafe { &*self.inner.get() },
                                borrow,
                            })
                        }
                        Err(err) => Err(err),
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
    pub(crate) fn borrow_mut(&self) -> Result<RefMut<'_, &'_ mut T>, error::Borrow> {
        #[cfg(feature = "thread_local")]
        {
            // if Sync - accessible from any thread, shared only if not world thread
            // if !Sync - accessible from world thread only
            if let Some(thread_id) = self.send {
                if thread_id != std::thread::current().id() {
                    return Err(error::Borrow::WrongThread);
                }
            }
        }

        match self.borrow_state.write() {
            Ok(borrow) => {
                Ok(RefMut {
                    // SAFE we locked
                    inner: unsafe { &mut *self.inner.get() },
                    borrow,
                })
            }
            Err(err) => Err(err),
        }
    }
    #[inline]
    pub(crate) fn get_mut(&mut self) -> &'_ mut T {
        self.inner.get_mut()
    }
}

/// Wraps an `AtomicRefcell`'s shared borrow.
pub struct Ref<'a, T> {
    inner: T,
    borrow: SharedBorrow<'a>,
}

impl<'a, T> Ref<'a, T> {
    /// Returns the inner parts of the `Ref`.
    ///
    /// # Safety
    ///
    /// The inner value and everything borrowing it must be dropped before `SharedBorrow`.
    #[inline]
    pub unsafe fn destructure(this: Self) -> (T, SharedBorrow<'a>) {
        (this.inner, this.borrow)
    }
}

impl<'a, T: ?Sized> Ref<'a, &'a T> {
    #[inline]
    pub(crate) fn map<U, F: FnOnce(&T) -> &U>(this: Self, f: F) -> Ref<'a, &'a U> {
        Ref {
            inner: f(this.inner),
            borrow: this.borrow,
        }
    }
}

impl<'a, T: Deref> Deref for Ref<'a, T> {
    type Target = T::Target;

    #[inline]
    fn deref(&self) -> &T::Target {
        self.inner.deref()
    }
}

impl<T: Clone> Clone for Ref<'_, T> {
    #[inline]
    fn clone(&self) -> Self {
        Ref {
            inner: self.inner.clone(),
            borrow: self.borrow.clone(),
        }
    }
}

/// Wraps an `AtomicRefcell`'s exclusive borrow.
pub struct RefMut<'a, T> {
    inner: T,
    borrow: ExclusiveBorrow<'a>,
}

impl<'a, T> RefMut<'a, T> {
    /// Returns the inner parts of the `RefMut`.
    ///
    /// # Safety
    ///
    /// The inner value and everything borrowing it must be dropped before `ExclusiveBorrow`.
    #[inline]
    pub unsafe fn destructure(this: Self) -> (T, ExclusiveBorrow<'a>) {
        (this.inner, this.borrow)
    }
}

impl<'a, T: ?Sized> RefMut<'a, &'a mut T> {
    #[inline]
    pub(crate) fn map<U, F: FnOnce(&mut T) -> &mut U>(this: Self, f: F) -> RefMut<'a, &'a mut U> {
        RefMut {
            inner: f(this.inner),
            borrow: this.borrow,
        }
    }
}

impl<'a, T: Deref> Deref for RefMut<'a, T> {
    type Target = T::Target;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T: DerefMut> DerefMut for RefMut<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[test]
fn shared() {
    let refcell = AtomicRefCell::new(0);
    let first_borrow = refcell.borrow().unwrap();

    assert!(refcell.borrow().is_ok());
    assert_eq!(refcell.borrow_mut().err(), Some(error::Borrow::Shared));

    drop(first_borrow);

    assert!(refcell.borrow_mut().is_ok());
}

#[test]
fn exclusive() {
    let refcell = AtomicRefCell::new(0);
    let first_borrow = refcell.borrow_mut().unwrap();

    assert_eq!(refcell.borrow().err(), Some(error::Borrow::Unique));
    assert_eq!(refcell.borrow_mut().err(), Some(error::Borrow::Unique));

    drop(first_borrow);

    assert!(refcell.borrow_mut().is_ok());
}

#[cfg(all(feature = "std", not(feature = "thread_local")))]
#[test]
fn shared_thread() {
    use alloc::sync::Arc;

    let refcell = Arc::new(AtomicRefCell::new(0));
    let refcell_clone = refcell.clone();
    let first_borrow = refcell.borrow().unwrap();

    std::thread::spawn(move || {
        refcell_clone.borrow().unwrap();
        assert_eq!(
            refcell_clone.borrow_mut().err(),
            Some(error::Borrow::Shared)
        );
    })
    .join()
    .unwrap();

    drop(first_borrow);

    assert!(refcell.borrow_mut().is_ok());
}

#[cfg(all(feature = "std", not(feature = "thread_local")))]
#[test]
fn exclusive_thread() {
    use std::sync::Arc;

    let refcell = Arc::new(AtomicRefCell::new(0));
    let refcell_clone = refcell.clone();

    std::thread::spawn(move || {
        let _first_borrow = refcell_clone.borrow_mut();
        assert_eq!(
            refcell_clone.borrow_mut().err(),
            Some(error::Borrow::Unique)
        );
    })
    .join()
    .unwrap();

    refcell.borrow_mut().unwrap();
}

#[cfg(feature = "thread_local")]
#[test]
fn non_send() {
    let refcell = AtomicRefCell::new_non_send(0u32, std::thread::current().id());
    let refcell_ptr: *const _ = &refcell;
    let refcell_ptr = refcell_ptr as usize;

    std::thread::spawn(move || unsafe {
        (&*(refcell_ptr as *const AtomicRefCell<u32>))
            .borrow()
            .unwrap();
        assert_eq!(
            (&*(refcell_ptr as *const AtomicRefCell<u32>))
                .borrow_mut()
                .err(),
            Some(error::Borrow::WrongThread)
        );
    })
    .join()
    .unwrap();

    refcell.borrow().unwrap();
    refcell.borrow_mut().unwrap();
}

#[cfg(feature = "thread_local")]
#[test]
fn non_sync() {
    let refcell = AtomicRefCell::new_non_sync(0);

    let refcell_ptr: *const _ = &refcell;
    let refcell_ptr = refcell_ptr as usize;

    std::thread::spawn(move || unsafe {
        (&*(refcell_ptr as *const AtomicRefCell<u32>))
            .borrow()
            .unwrap();
        (&*(refcell_ptr as *const AtomicRefCell<u32>))
            .borrow_mut()
            .unwrap();
    })
    .join()
    .unwrap();

    refcell.borrow().unwrap();
    refcell.borrow_mut().unwrap();
}

#[cfg(feature = "thread_local")]
#[test]
fn non_send_sync() {
    let refcell = AtomicRefCell::new_non_send_sync(0u32, std::thread::current().id());
    let refcell_ptr: *const _ = &refcell;
    let refcell_ptr = refcell_ptr as usize;

    std::thread::spawn(move || unsafe {
        assert_eq!(
            (&*(refcell_ptr as *const AtomicRefCell<u32>))
                .borrow()
                .err(),
            Some(error::Borrow::WrongThread)
        );
        assert_eq!(
            (&*(refcell_ptr as *const AtomicRefCell<u32>))
                .borrow_mut()
                .err(),
            Some(error::Borrow::WrongThread)
        );
    })
    .join()
    .unwrap();

    refcell.borrow().unwrap();
    refcell.borrow_mut().unwrap();
}
