use std::any::TypeId;

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

/// Error related to adding an entity.
/// AllStoragesBorrow means an add_storage operation is in progress.
/// Entities means entities is already borrowed.
#[derive(Debug)]
pub enum NewEntity {
    AllStoragesBorrow(Borrow),
    Entities(Borrow),
}

/// Trying to pack a storage twice will result in this error.
#[derive(Debug)]
pub enum Pack {
    // `TypeId` of the problematic storage
    AlreadyPacked(TypeId),
}

/// If a storage is packed_owned all storages packed with it have to be
/// passed in the add_component call even if no components are added.
#[derive(Debug)]
pub enum AddComponent {
    // `TypeId` of the storage requirering more storages
    MissingPackStorage(TypeId),
}

#[derive(Debug)]
pub enum WorldPack {
    GetStorage(GetStorage),
    Pack(Pack),
}

impl From<GetStorage> for WorldPack {
    fn from(get_storage: GetStorage) -> Self {
        WorldPack::GetStorage(get_storage)
    }
}

impl From<Pack> for WorldPack {
    fn from(pack: Pack) -> Self {
        WorldPack::Pack(pack)
    }
}

/// When removing components if one of them is packed owned, all storages packed
/// with it must be passed to the function.
/// This error occurs when there is a missing storage, `TypeId` will indicate which storage.
#[derive(Debug)]
pub enum Remove {
    MissingPackStorage(TypeId),
}
