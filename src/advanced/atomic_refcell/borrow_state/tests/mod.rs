use super::*;

#[test]
fn reborrow() {
    let borrow = BorrowState::new();
    let write = borrow.write().unwrap();

    let read = write.shared_reborrow();

    assert_eq!(HIGH_BIT + 1, (read.0).0.load(Ordering::Relaxed));
}
