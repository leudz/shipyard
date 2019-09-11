use std::alloc::{alloc, dealloc, Layout};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) struct ParBuf<T> {
    pub(super) len: AtomicUsize,
    cap: usize,
    pub(super) buf: *mut T,
    _phantom: PhantomData<T>,
}

unsafe impl<T: Send> Send for ParBuf<T> {}
unsafe impl<T: Send> Sync for ParBuf<T> {}

impl<T> ParBuf<T> {
    pub(super) fn new(size: usize) -> Self {
        let layout = Layout::new::<T>();
        let layout = Layout::from_size_align(layout.size() * size, layout.align()).unwrap();
        let ptr = unsafe { alloc(layout) };

        ParBuf {
            len: AtomicUsize::new(0),
            cap: size,
            buf: ptr as _,
            _phantom: PhantomData,
        }
    }
    pub(super) fn push(&self, item: T) {
        let index = self.len.fetch_add(1, Ordering::Release);
        assert!(index < self.cap);
        unsafe { self.buf.add(index).write(item) };
    }
}

impl<T> Drop for ParBuf<T> {
    fn drop(&mut self) {
        let layout = Layout::new::<T>();
        let layout = Layout::from_size_align(layout.size() * self.cap, layout.align()).unwrap();
        unsafe { dealloc(self.buf as _, layout) };
    }
}

#[test]
fn sequential() {
    let buffer = ParBuf::new(10);

    for i in 0..10 {
        buffer.push(i);
    }

    for i in 0..10 {
        assert_eq!(unsafe { buffer.buf.add(i).read() }, i);
    }
}

#[test]
fn parallel() {
    use rayon::prelude::*;

    let buffer: ParBuf<i32> = ParBuf::new(1000);

    (0..1000).into_par_iter().for_each(|i| {
        buffer.push(i);
    });

    assert_eq!(buffer.len.load(Ordering::Relaxed), 1000);

    let slice = unsafe { &*(buffer.buf as *mut [i32; 1000]) };

    for i in 0..1000 {
        assert!(slice.contains(&i));
    }
}

#[test]
fn partial_parallel() {
    use rayon::prelude::*;

    let buffer: ParBuf<i32> = ParBuf::new(1000);

    (0..500).into_par_iter().for_each(|i| {
        buffer.push(i);
    });

    assert_eq!(buffer.len.load(Ordering::Relaxed), 500);

    let slice = unsafe { &*(buffer.buf as *mut [i32; 500]) };

    for i in 0..500 {
        assert!(slice.contains(&i));
    }
}
