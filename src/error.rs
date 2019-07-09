/// AtomicRefCell's borrow error.
/// Unique means the BorrowState was mutably borrowed when an illegal borrow occured.
/// Shared means the BorrowState was immutably borrowed when an illegal borrow occured.
#[derive(Debug)]
pub enum Borrow {
    Unique,
    Shared,
}
