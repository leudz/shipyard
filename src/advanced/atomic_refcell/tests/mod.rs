mod miri;

use super::*;

#[test]
fn shared() {
    let refcell = AtomicRefCell::new(0);
    let first_borrow = refcell.borrow().unwrap();

    assert!(refcell.borrow().is_ok());
    assert_eq!(refcell.borrow_mut().err(), Some(error::Borrow::Unique));

    drop(first_borrow);

    assert!(refcell.borrow_mut().is_ok());
}

#[test]
fn exclusive() {
    let refcell = AtomicRefCell::new(0);
    let first_borrow = refcell.borrow_mut().unwrap();

    assert_eq!(refcell.borrow().err(), Some(error::Borrow::Shared));
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
            Some(error::Borrow::Unique)
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

#[cfg(all(feature = "std", feature = "thread_local"))]
#[test]
fn non_send() {
    use crate::std_thread_id_generator;

    let refcell = AtomicRefCell::new_non_send(0u32, Arc::new(std_thread_id_generator));
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

#[cfg(all(feature = "std", feature = "thread_local"))]
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

#[cfg(all(feature = "std", feature = "thread_local"))]
#[test]
fn non_send_sync() {
    use crate::std_thread_id_generator;

    let refcell = AtomicRefCell::new_non_send_sync(0u32, Arc::new(std_thread_id_generator));
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
