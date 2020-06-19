use crate::error;
use alloc::boxed::Box;
use core::cell::UnsafeCell;
use core::mem::ManuallyDrop;
use core::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "std")]
use std::thread;
#[cfg(feature = "std")]
use thread::ThreadId;

/// Threadsafe `RefCell`-like container.
#[doc(hidden)]
pub struct AtomicRefCell<T: ?Sized> {
    borrow_state: BorrowState,
    #[cfg(feature = "std")]
    send: Option<ThreadId>,
    #[cfg(feature = "std")]
    is_sync: bool,
    _non_send: core::marker::PhantomData<*const ()>,
    taken: bool,
    inner: ManuallyDrop<UnsafeCell<T>>,
}

// AtomicRefCell can't be Send if it contains !Send components
#[cfg(not(feature = "non_send"))]
unsafe impl<T: ?Sized> Send for AtomicRefCell<T> {}

unsafe impl<T: ?Sized> Sync for AtomicRefCell<T> {}

impl<T: ?Sized> Drop for AtomicRefCell<T> {
    fn drop(&mut self) {
        if !self.taken {
            // SAFE we're in the Drop impl so it won't be accessed again
            unsafe {
                ManuallyDrop::drop(&mut self.inner);
            }
        }
    }
}

impl<T: ?Sized> AtomicRefCell<T> {
    /// Creates a new `AtomicRefCell` containing `value`.
    pub(crate) fn new(
        value: T,
        #[cfg(feature = "std")] send: Option<ThreadId>,
        #[cfg(feature = "std")] is_sync: bool,
    ) -> Self
    where
        T: Sized,
    {
        AtomicRefCell {
            borrow_state: Default::default(),
            #[cfg(feature = "std")]
            send,
            #[cfg(feature = "std")]
            is_sync,
            _non_send: core::marker::PhantomData,
            taken: false,
            inner: ManuallyDrop::new(UnsafeCell::new(value)),
        }
    }
    /// Immutably borrows the wrapped value, returning an error if the value is currently mutably
    /// borrowed.
    ///
    /// The borrow lasts until the returned `Ref` exits scope. Multiple shared borrows can be
    /// taken out at the same time.
    pub(crate) fn try_borrow(&self) -> Result<Ref<'_, T>, error::Borrow> {
        #[cfg(feature = "std")]
        {
            Ok(Ref {
                borrow: self.borrow_state.try_borrow(self.send, self.is_sync)?,
                // SAFE we have the lock
                inner: unsafe { &*self.inner.get() },
            })
        }
        #[cfg(not(feature = "std"))]
        {
            Ok(Ref {
                borrow: self.borrow_state.try_borrow()?,
                // SAFE we have the lock
                inner: unsafe { &*self.inner.get() },
            })
        }
    }
    /// Mutably borrows the wrapped value, returning an error if the value is currently borrowed.
    ///
    /// The borrow lasts until the returned `RefMut` exits scope. The value cannot be borrowed while this borrow is
    /// active.
    pub(crate) fn try_borrow_mut(&self) -> Result<RefMut<'_, T>, error::Borrow> {
        #[cfg(feature = "std")]
        {
            Ok(RefMut {
                borrow: self.borrow_state.try_borrow_mut(self.send, self.is_sync)?,
                // SAFE we have the lock
                inner: unsafe { &mut *self.inner.get() },
            })
        }
        #[cfg(not(feature = "std"))]
        {
            Ok(RefMut {
                borrow: self.borrow_state.try_borrow_mut()?,
                // SAFE we have the lock
                inner: unsafe { &mut *self.inner.get() },
            })
        }
    }
}

impl AtomicRefCell<dyn crate::unknown_storage::UnknownStorage> {
    /// # Safety
    ///
    /// `T` has to be a unique storage of the right type.
    #[allow(clippy::boxed_local)]
    pub unsafe fn into_unique<T: 'static>(mut this: Box<Self>) -> T {
        use core::mem::MaybeUninit;

        let mut tmp: MaybeUninit<T> = MaybeUninit::uninit();

        this.taken = true;
        // SAFE both regions are valids
        tmp.as_mut_ptr()
            .copy_from_nonoverlapping((&*this.inner.get()).unique::<T>().unwrap(), 1);

        // SAFE this is initialized
        tmp.assume_init()
    }
}

/// `BorrowState` keeps track of which borrow is currently active.
// If `HIGH_BIT` is set, it is a unique borrow, in all other cases it is a shared borrowed
#[doc(hidden)]
pub struct BorrowState(AtomicUsize);

const HIGH_BIT: usize = !(core::usize::MAX >> 1);
const MAX_FAILED_BORROWS: usize = HIGH_BIT + (HIGH_BIT >> 1);

impl BorrowState {
    // Each borrow will add one, check if no unique borrow is active before returning
    // Even in case of failure the incrementation leave the value in a valid state
    pub(crate) fn try_borrow(
        &self,
        #[cfg(feature = "std")] send: Option<ThreadId>,
        #[cfg(feature = "std")] is_sync: bool,
    ) -> Result<Borrow<'_>, error::Borrow> {
        #[cfg(feature = "std")]
        {
            match (send, is_sync) {
                (None, true) => {
                    // accessible from any thread, shared xor unique
                    let new = self.0.fetch_add(1, Ordering::Acquire) + 1;

                    if new & HIGH_BIT != 0 {
                        Err(Self::try_recover(self, new))
                    } else {
                        Ok(Borrow::Shared(self))
                    }
                }
                (None, false) => {
                    // accessible from one thread at a time
                    match self
                        .0
                        .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
                    {
                        Ok(_) => Ok(Borrow::Shared(self)),
                        _ => Err(error::Borrow::MultipleThreads),
                    }
                }
                (Some(_), true) => {
                    // accessible from any thread, shared only if not original thread
                    let new = self.0.fetch_add(1, Ordering::Acquire) + 1;

                    if new & HIGH_BIT != 0 {
                        Err(Self::try_recover(self, new))
                    } else {
                        Ok(Borrow::Shared(self))
                    }
                }
                (Some(thread_id), false) => {
                    // accessible from origianl thread only
                    if thread_id == thread::current().id() {
                        let new = self.0.fetch_add(1, Ordering::Acquire) + 1;

                        if new & HIGH_BIT != 0 {
                            Err(Self::try_recover(self, new))
                        } else {
                            Ok(Borrow::Shared(self))
                        }
                    } else {
                        Err(error::Borrow::WrongThread)
                    }
                }
            }
        }
        #[cfg(not(feature = "std"))]
        {
            let new = self.0.fetch_add(1, Ordering::Acquire) + 1;

            if new & HIGH_BIT != 0 {
                Err(Self::try_recover(self, new))
            } else {
                Ok(Borrow::Shared(self))
            }
        }
    }
    // Can only make a unique borrow when no borrows are active
    // Use `compare_exchange` to keep the value in a valid state even in case of failure
    pub(crate) fn try_borrow_mut(
        &self,
        #[cfg(feature = "std")] send: Option<ThreadId>,
        #[cfg(feature = "std")] is_sync: bool,
    ) -> Result<Borrow<'_>, error::Borrow> {
        #[cfg(feature = "std")]
        {
            match (send, is_sync) {
                (None, true) | (None, false) => {
                    // accessible from one thread at a time
                    match self
                        .0
                        .compare_exchange(0, HIGH_BIT, Ordering::Acquire, Ordering::Relaxed)
                    {
                        Ok(_) => Ok(Borrow::Unique(self)),
                        _ => Err(error::Borrow::Unique),
                    }
                }
                (Some(thread_id), true) | (Some(thread_id), false) => {
                    // accessible from origianl thread only
                    if thread_id == thread::current().id() {
                        match self.0.compare_exchange(
                            0,
                            HIGH_BIT,
                            Ordering::Acquire,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => Ok(Borrow::Unique(self)),
                            _ => Err(error::Borrow::Unique),
                        }
                    } else {
                        Err(error::Borrow::WrongThread)
                    }
                }
            }
        }
        #[cfg(not(feature = "std"))]
        {
            match self
                .0
                .compare_exchange(0, HIGH_BIT, Ordering::Acquire, Ordering::Relaxed)
            {
                Ok(_) => Ok(Borrow::Unique(self)),
                _ => Err(error::Borrow::Unique),
            }
        }
    }
    // In case of a failled shared borrow, check all possible causes and recover from it when possible
    // If `new == HIGH_BIT` there is `isize::MAX` active or forgotten shared borrows
    // If `new >= MAX_FAILED_BORROWS` there is a unique borrows and `isize::MAX` attenpts to borrow immutably
    // In all other cases, a unique borrow is active
    fn try_recover(&self, new: usize) -> error::Borrow {
        if new == HIGH_BIT {
            self.0.fetch_sub(1, Ordering::Release);
            panic!("Too many shared borrows");
        } else if new >= MAX_FAILED_BORROWS {
            #[cfg(feature = "std")]
            {
                println!("Too many failed borrows");
                std::process::exit(1);
            }
            #[cfg(not(feature = "std"))]
            {
                // This isn't great but I don't know better
                panic!("Too many failed borrows");
            }
        } else {
            // Tries to go back to the previous state, even if it fails the state is still valid
            // Going back only allow more tries before hitting `MAX_FAILED_BORROWS`
            let _ = self
                .0
                .compare_exchange(new, new - 1, Ordering::Release, Ordering::Relaxed);
            error::Borrow::Shared
        }
    }
}

impl Default for BorrowState {
    fn default() -> Self {
        BorrowState(AtomicUsize::new(0))
    }
}

#[doc(hidden)]
pub enum Borrow<'a> {
    Shared(&'a BorrowState),
    Unique(&'a BorrowState),
    None,
}

impl Clone for Borrow<'_> {
    fn clone(&self) -> Self {
        match self {
            Borrow::Shared(borrow) => {
                #[cfg(feature = "std")]
                {
                    borrow.try_borrow(None, true).unwrap()
                }
                #[cfg(not(feature = "std"))]
                {
                    borrow.try_borrow().unwrap()
                }
            }
            Borrow::Unique(_) => panic!("Can't clone a unique borrow."),
            Borrow::None => Borrow::None,
        }
    }
}

impl<'a> Drop for Borrow<'a> {
    fn drop(&mut self) {
        match self {
            Borrow::Shared(borrow) => {
                let old = borrow.0.fetch_sub(1, Ordering::Release);
                debug_assert!(old & HIGH_BIT == 0);
            }
            Borrow::Unique(borrow) => {
                borrow.0.store(0, Ordering::Release);
            }
            Borrow::None => {}
        }
    }
}

/// A wrapper type for a shared borrow from a `AtomicRefCell<T>`.
pub struct Ref<'a, T: ?Sized> {
    pub(crate) inner: &'a T,
    pub(crate) borrow: Borrow<'a>,
}

impl<'a, T: 'a + ?Sized> Ref<'a, T> {
    /// Makes a new `Ref` for a component of the borrowed data.
    pub(crate) fn map<U, F>(origin: Self, f: F) -> Ref<'a, U>
    where
        F: FnOnce(&T) -> &U,
    {
        Ref {
            inner: f(origin.inner),
            borrow: origin.borrow,
        }
    }
    /// Makes a new `Ref` for a component of the borrowed data, the operation can fail.
    pub(crate) fn try_map<U, E, F>(origin: Self, f: F) -> Result<Ref<'a, U>, E>
    where
        F: FnOnce(&T) -> Result<&U, E>,
    {
        Ok(Ref {
            inner: f(origin.inner)?,
            borrow: origin.borrow,
        })
    }
    /// Get the inner parts of the `Ref`.
    ///
    /// # Safety
    ///
    /// The reference has to be dropped before `Borrow`.
    pub(crate) unsafe fn destructure(Ref { inner, borrow, .. }: Self) -> (&'a T, Borrow<'a>) {
        (inner, borrow)
    }
}

impl<T: ?Sized> core::ops::Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.inner
    }
}

impl<T: ?Sized> AsRef<T> for Ref<'_, T> {
    fn as_ref(&self) -> &T {
        self.inner
    }
}

impl<T: ?Sized> Clone for Ref<'_, T> {
    fn clone(&self) -> Self {
        Ref {
            inner: self.inner,
            borrow: self.borrow.clone(),
        }
    }
}

/// A wrapper type for a unique borrow from a `AtomicRefCell<T>`.
pub struct RefMut<'a, T: ?Sized> {
    pub(crate) inner: &'a mut T,
    pub(crate) borrow: Borrow<'a>,
}

impl<'a, T: 'a + ?Sized> RefMut<'a, T> {
    /// Makes a new `RefMut` for a component of the borrowed data.
    pub(crate) fn map<U, F>(origin: Self, f: F) -> RefMut<'a, U>
    where
        F: FnOnce(&mut T) -> &mut U,
    {
        RefMut {
            inner: f(origin.inner),
            borrow: origin.borrow,
        }
    }
    /// Makes a new `RefMut` for a component of the borrowed data, the operation can fail.
    pub(crate) fn try_map<U, E, F>(origin: Self, f: F) -> Result<RefMut<'a, U>, E>
    where
        F: FnOnce(&mut T) -> Result<&mut U, E>,
    {
        Ok(RefMut {
            inner: f(origin.inner)?,
            borrow: origin.borrow,
        })
    }
}

impl<T: ?Sized> core::ops::Deref for RefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.inner
    }
}

impl<T: ?Sized> core::ops::DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.inner
    }
}

impl<T: ?Sized> AsRef<T> for RefMut<'_, T> {
    fn as_ref(&self) -> &T {
        self.inner
    }
}

impl<T: ?Sized> AsMut<T> for RefMut<'_, T> {
    fn as_mut(&mut self) -> &mut T {
        self.inner
    }
}

#[test]
fn reborrow() {
    let refcell = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(0, None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(0)
        }
    };
    let _first_borrow = refcell.try_borrow().unwrap();

    assert!(refcell.try_borrow().is_ok());
    assert_eq!(
        std::mem::discriminant(&refcell.try_borrow_mut().err().unwrap()),
        std::mem::discriminant(&error::Borrow::Unique)
    );
}
#[test]
fn unique_reborrow() {
    let refcell = {
        #[cfg(feature = "std")]
        {
            AtomicRefCell::new(0, None, true)
        }
        #[cfg(not(feature = "std"))]
        {
            AtomicRefCell::new(0)
        }
    };
    let _first_borrow = refcell.try_borrow_mut().unwrap();

    assert_eq!(
        std::mem::discriminant(&refcell.try_borrow().err().unwrap()),
        std::mem::discriminant(&error::Borrow::Shared)
    );
    assert_eq!(
        std::mem::discriminant(&refcell.try_borrow_mut().err().unwrap()),
        std::mem::discriminant(&error::Borrow::Unique)
    );
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn non_send_sync() {
    struct Test(*const ());
    unsafe impl Sync for Test {}

    let refcell = AtomicRefCell::new(Test(&()), Some(thread::current().id()), true);
    refcell.try_borrow_mut().unwrap();
    let borrow = refcell.try_borrow();
    rayon::scope(|_| {
        refcell.try_borrow().unwrap();
    });
    drop(borrow);
    rayon::scope(|_| {
        assert_eq!(
            refcell.try_borrow_mut().err(),
            Some(error::Borrow::WrongThread)
        );
    });
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn non_sync_send() {
    struct Test(*const ());
    unsafe impl Send for Test {}

    let refcell = AtomicRefCell::new(Test(&()), None, false);
    refcell.try_borrow_mut().unwrap();
    let borrow = refcell.try_borrow().unwrap();
    rayon::scope(|_| {
        assert_eq!(
            refcell.try_borrow().err(),
            Some(error::Borrow::MultipleThreads)
        );
    });
    rayon::scope(|_| {
        assert_eq!(refcell.try_borrow_mut().err(), Some(error::Borrow::Unique));
    });
    drop(borrow);
    rayon::scope(|_| {
        refcell.try_borrow().unwrap();
        refcell.try_borrow_mut().unwrap();
    });
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn non_send_non_sync() {
    struct Test(*const ());

    let refcell = AtomicRefCell::new(Test(&()), Some(thread::current().id()), false);
    refcell.try_borrow_mut().unwrap();
    refcell.try_borrow().unwrap();
    rayon::scope(|_| {
        assert_eq!(refcell.try_borrow().err(), Some(error::Borrow::WrongThread));
    });
    rayon::scope(|_| {
        assert_eq!(
            refcell.try_borrow_mut().err(),
            Some(error::Borrow::WrongThread)
        );
    });
}
