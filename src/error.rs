/// AtomicRefCell's borrow error.
/// Unique means the BorrowState was mutably borrowed when an illegal borrow occured.
/// Shared means the BorrowState was immutably borrowed when an illegal borrow occured.
#[derive(Debug)]
pub enum Borrow {
    Unique,
    Shared,
}

/// Error related to acquiring a storage.
/// AllStoragesBorrow means an add_storage operation is in progress.
/// StorageBorrow means this storage is already borrowed.
/// MissingComponent signify no storage exists for this type.
#[derive(Debug)]
pub enum GetStorage {
    AllStoragesBorrow(Borrow),
    StorageBorrow(Borrow),
    MissingComponent,
}
