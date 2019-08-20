use std::any::TypeId;
use std::fmt::{Debug, Display, Formatter};

/// AtomicRefCell's borrow error.
///
/// Unique means the BorrowState was mutably borrowed when an illegal borrow occured.
///
/// Shared means the BorrowState was immutably borrowed when an illegal borrow occured.
pub enum Borrow {
    Unique,
    Shared,
}

impl Debug for Borrow {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Borrow::Unique => fmt.write_str("Cannot mutably borrow while already borrowed."),
            Borrow::Shared => {
                fmt.write_str("Cannot immutably borrow while already mutably borrowed.")
            }
        }
    }
}

impl Display for Borrow {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Error related to acquiring a storage.
///
/// AllStoragesBorrow means an add_storage operation is in progress.
///
/// StorageBorrow means this storage is already borrowed.
///
/// MissingComponent signify no storage exists for this type.
pub enum GetStorage {
    AllStoragesBorrow(Borrow),
    StorageBorrow(Borrow),
    MissingComponent,
}

impl Debug for GetStorage {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            GetStorage::AllStoragesBorrow(borrow) => match borrow {
                Borrow::Unique => fmt.write_str("Cannot mutably borrow all storages while it's already borrowed (this include component storage)."),
                Borrow::Shared => {
                    fmt.write_str("Cannot immutably borrow all storages while it's already mutably borrowed.")
                }
            },
            GetStorage::StorageBorrow(borrow) => match borrow {
                Borrow::Unique => fmt.write_str("Cannot mutably borrow a storage while it's already borrowed."),
                Borrow::Shared => {
                    fmt.write_str("Cannot immutably borrow a storage while it's already mutably borrowed.")
                }
            },
            GetStorage::MissingComponent => fmt.write_str("No storage with this type exists.")
        }
    }
}

impl Display for GetStorage {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Error related to adding an entity.
///
/// AllStoragesBorrow means an add_storage operation is in progress.
///
/// Entities means entities is already borrowed.
pub enum NewEntity {
    AllStoragesBorrow(Borrow),
    Entities(Borrow),
}

impl Debug for NewEntity {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            NewEntity::AllStoragesBorrow(borrow) => match borrow {
                Borrow::Unique => fmt.write_str("Cannot mutably borrow all storages while it's already borrowed (this include component storage)."),
                Borrow::Shared => {
                    fmt.write_str("Cannot immutably borrow all storages while it's already mutably borrowed.")
                }
            },
            NewEntity::Entities(borrow) => match borrow {
                Borrow::Unique => fmt.write_str("Cannot mutably borrow entities while it's already borrowed."),
                Borrow::Shared => unreachable!(),
            },
        }
    }
}

impl Display for NewEntity {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// If a storage is packed_owned all storages packed with it have to be
/// passed in the add_component call even if no components are added.
pub enum AddComponent {
    // `TypeId` of the storage requirering more storages
    MissingPackStorage(TypeId),
    EntityIsNotAlive,
}

impl Debug for AddComponent {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            AddComponent::MissingPackStorage(type_id) => fmt.write_fmt(format_args!("Missing storage for type ({:?}). To add a packed component you have to pass all storages packed with it. Even if you just add one component.", type_id)),
            AddComponent::EntityIsNotAlive => fmt.write_str("Entity has to be alive to add component to it."),
        }
    }
}

impl Display for AddComponent {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Error related to [World::pack_owned] and [World::try_pack_owned].
///
/// [World::pack_owned]: ../struct.World.html#method.pack_owned
/// [World::try_pack_owned]: ../struct.World.html#method.try_pack_owned
pub enum Pack {
    GetStorage(GetStorage),
    AlreadyTightPack(TypeId),
    AlreadyLoosePack(TypeId),
}

impl From<GetStorage> for Pack {
    fn from(get_storage: GetStorage) -> Self {
        Pack::GetStorage(get_storage)
    }
}

impl Debug for Pack {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Pack::GetStorage(get_storage) => Debug::fmt(get_storage, fmt),
            Pack::AlreadyTightPack(type_id) => fmt.write_fmt(format_args!(
                "The storage of type ({:?}) is already tightly packed.",
                type_id
            )),
            Pack::AlreadyLoosePack(type_id) => fmt.write_fmt(format_args!(
                "The storage of type ({:?}) is already loosely packed.",
                type_id
            )),
        }
    }
}

impl Display for Pack {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// When removing components if one of them is packed owned, all storages packed
/// with it must be passed to the function.
///
/// This error occurs when there is a missing storage, `TypeId` will indicate which storage.
pub enum Remove {
    MissingPackStorage(TypeId),
}

impl Debug for Remove {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Remove::MissingPackStorage(type_id) => fmt.write_fmt(format_args!("Missing storage for type ({:?}). To remove a packed component you have to pass all storages packed with it. Even if you just remove one component.", type_id))
        }
    }
}

impl Display for Remove {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Trying to set the default workload to a non existant one will result in this error.
pub enum SetDefaultWorkload {
    Borrow(Borrow),
    MissingWorkload,
}

impl From<Borrow> for SetDefaultWorkload {
    fn from(borrow: Borrow) -> Self {
        SetDefaultWorkload::Borrow(borrow)
    }
}

impl Debug for SetDefaultWorkload {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            SetDefaultWorkload::Borrow(borrow) => match borrow {
                Borrow::Unique => {
                    fmt.write_str("Cannot mutably borrow pipeline while it's already borrowed.")
                }
                Borrow::Shared => unreachable!(),
            },
            SetDefaultWorkload::MissingWorkload => {
                fmt.write_str("No workload with this name exists.")
            }
        }
    }
}

impl Display for SetDefaultWorkload {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Try to run a non existant workload.
pub enum RunWorkload {
    Borrow(Borrow),
    MissingWorkload,
}

impl From<Borrow> for RunWorkload {
    fn from(borrow: Borrow) -> Self {
        RunWorkload::Borrow(borrow)
    }
}

impl Debug for RunWorkload {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        match self {
            RunWorkload::Borrow(borrow) => match borrow {
                Borrow::Unique => unreachable!(),
                Borrow::Shared => {
                    fmt.write_str("Cannot mutably borrow pipeline while it's already borrowed.")
                }
            },
            RunWorkload::MissingWorkload => fmt.write_str("No workload with this name exists."),
        }
    }
}

impl Display for RunWorkload {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}
