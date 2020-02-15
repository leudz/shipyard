use crate::EntityId;
use core::fmt::{Debug, Display, Formatter};
#[cfg(feature = "std")]
use std::error::Error;

/// AtomicRefCell's borrow error.
///
/// Unique means the BorrowState was mutably borrowed when an illegal borrow occured.
///
/// Shared means the BorrowState was immutably borrowed when an illegal borrow occured.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Borrow {
    Unique,
    Shared,
    WrongThread,
    MultipleThreads,
}

#[cfg(feature = "std")]
impl Error for Borrow {}

impl Debug for Borrow {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::Unique => fmt.write_str("Cannot mutably borrow while already borrowed."),
            Self::Shared => {
                fmt.write_str("Cannot immutably borrow while already mutably borrowed.")
            }
            Self::WrongThread => {
                fmt.write_str("Can't access from another thread because it's !Send and !Sync.")
            }
            Self::MultipleThreads => fmt.write_str(
                "Can't access from multiple threads at the same time because it's !Sync.",
            ),
        }
    }
}

impl Display for Borrow {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Error related to acquiring a storage.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GetStorage {
    AllStoragesBorrow(Borrow),
    StorageBorrow((&'static str, Borrow)),
    NonUnique((&'static str, Borrow)),
    MissingUnique(&'static str),
    Entities(Borrow),
}

#[cfg(feature = "std")]
impl Error for GetStorage {}

impl Debug for GetStorage {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::AllStoragesBorrow(borrow) => match borrow {
                Borrow::Unique => fmt.write_str("Cannot mutably borrow AllStorages while it's already borrowed (AllStorages is borrowed to access any storage)."),
                Borrow::Shared => {
                    fmt.write_str("Cannot immutably borrow AllStorages while it's already mutably borrowed.")
                },
                _ => unreachable!(),
            },
            Self::StorageBorrow((name, borrow)) => match borrow {
                Borrow::Unique => fmt.write_fmt(format_args!("Cannot mutably borrow {} storage while it's already borrowed.", name)),
                Borrow::Shared => {
                    fmt.write_fmt(format_args!("Cannot immutably borrow {} storage while it's already mutably borrowed.", name))
                },
                Borrow::MultipleThreads => fmt.write_fmt(format_args!("Cannot borrow {} storage from multiple thread at the same time because it's !Sync.", name)),
                Borrow::WrongThread => fmt.write_fmt(format_args!("Cannot borrow {} storage from other thread than the one it was created in because it's !Send and !Sync.", name)),
            },
            Self::MissingUnique(name) => fmt.write_fmt(format_args!("No unique storage exists for {name}.\nConsider adding this line after the creation of World: world.register_unique::<{name}>(/* your_storage */);", name = name)),
            Self::NonUnique((name, mutation)) => match mutation {
                Borrow::Shared => fmt.write_fmt(format_args!("{name}'s storage isn't unique.\nYou might have forgotten to declare it, replace world.register::<{name}>() by world.register_unique(/* your_storage */).\nIf it isn't supposed to be a unique storage, replace Unique<&{name}> by &{name}.", name = name)),
                Borrow::Unique => fmt.write_fmt(format_args!("{name}'s storage isn't unique.\nYou might have forgotten to declare it, replace world.register::<{name}>() by world.register_unique(/* your_storage */).\nIf it isn't supposed to be a unique storage, replace Unique<&mut {name}> by &mut {name}.", name = name)),
                _ => unreachable!(),
            },
            Self::Entities(borrow) => match borrow {
                Borrow::Unique => fmt.write_str("Cannot mutably borrow Entities storage while it's already borrowed."),
                Borrow::Shared => {
                    fmt.write_str("Cannot immutably borrow Entities storage while it's already mutably borrowed.")
                },
                _ => unreachable!(),
            },
        }
    }
}

impl Display for GetStorage {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Error related to adding an entity.
///
/// AllStoragesBorrow means an add_storage operation is in progress.
///
/// Entities means entities is already borrowed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NewEntity {
    AllStoragesBorrow(Borrow),
    Entities(Borrow),
}

#[cfg(feature = "std")]
impl Error for NewEntity {}

impl Debug for NewEntity {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::AllStoragesBorrow(borrow) => match borrow {
                Borrow::Unique => fmt.write_str("Cannot mutably borrow all storages while it's already borrowed (this include component storage)."),
                Borrow::Shared => {
                    fmt.write_str("Cannot immutably borrow all storages while it's already mutably borrowed.")
                },
                _ => unreachable!(),
            },
            Self::Entities(borrow) => match borrow {
                Borrow::Unique => fmt.write_str("Cannot mutably borrow entities while it's already borrowed."),
                _ => unreachable!(),
            },
        }
    }
}

impl Display for NewEntity {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// If a storage is packed_owned all storages packed with it have to be
/// passed in the add_component call even if no components are added.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AddComponent {
    // `TypeId` of the storage requirering more storages
    MissingPackStorage(&'static str),
    EntityIsNotAlive,
}

#[cfg(feature = "std")]
impl Error for AddComponent {}

impl Debug for AddComponent {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::MissingPackStorage(type_id) => fmt.write_fmt(format_args!("Missing {} storage, to add a packed component you have to pass all storages packed with it. Even if you just add one component.", type_id)),
            Self::EntityIsNotAlive => fmt.write_str("Entity has to be alive to add component to it."),
        }
    }
}

impl Display for AddComponent {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Error occuring when a pack can't be made.
/// It could be a borrow issue or one of the storage could already have
/// an incompatible pack or the storage could be unique.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pack {
    GetStorage(GetStorage),
    AlreadyTightPack(&'static str),
    AlreadyLoosePack(&'static str),
    AlreadyUpdatePack(&'static str),
}

#[cfg(feature = "std")]
impl Error for Pack {}

impl From<GetStorage> for Pack {
    fn from(get_storage: GetStorage) -> Self {
        Pack::GetStorage(get_storage)
    }
}

impl Debug for Pack {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::GetStorage(get_storage) => Debug::fmt(get_storage, fmt),
            Self::AlreadyTightPack(type_name) => fmt.write_fmt(format_args!(
                "{} storage is already tightly packed.",
                type_name
            )),
            Self::AlreadyLoosePack(type_name) => fmt.write_fmt(format_args!(
                "{} storage is already loosely packed.",
                type_name
            )),
            Self::AlreadyUpdatePack(type_name) => fmt.write_fmt(format_args!(
                "{} storage is already has an update pack.",
                type_name
            )),
        }
    }
}

impl Display for Pack {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// When removing components if one of them is packed owned, all storages packed
/// with it must be passed to the function.
///
/// This error occurs when there is a missing storage, `TypeId` will indicate which storage.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Remove {
    MissingPackStorage(&'static str),
}

#[cfg(feature = "std")]
impl Error for Remove {}

impl Debug for Remove {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::MissingPackStorage(type_id) => fmt.write_fmt(format_args!("Missing {} storage, to remove a packed component you have to pass all storages packed with it. Even if you just remove one component.", type_id))
        }
    }
}

impl Display for Remove {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Trying to set the default workload to a non existant one will result in this error.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SetDefaultWorkload {
    Borrow(Borrow),
    MissingWorkload,
}

#[cfg(feature = "std")]
impl Error for SetDefaultWorkload {}

impl From<Borrow> for SetDefaultWorkload {
    fn from(borrow: Borrow) -> Self {
        SetDefaultWorkload::Borrow(borrow)
    }
}

impl Debug for SetDefaultWorkload {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::Borrow(borrow) => match borrow {
                Borrow::Unique => {
                    fmt.write_str("Cannot mutably borrow scheduler while it's already borrowed.")
                }
                _ => unreachable!(),
            },
            Self::MissingWorkload => fmt.write_str("No workload with this name exists."),
        }
    }
}

impl Display for SetDefaultWorkload {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Try to run a non existant workload.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunWorkload {
    Scheduler,
    GetStorage(GetStorage),
    MissingWorkload,
}

#[cfg(feature = "std")]
impl Error for RunWorkload {}

impl From<GetStorage> for RunWorkload {
    fn from(get_storage: GetStorage) -> Self {
        RunWorkload::GetStorage(get_storage)
    }
}

impl Debug for RunWorkload {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::Scheduler => {
                fmt.write_str("Cannot borrow scheduler while it's already mutably borrowed.")
            }
            Self::MissingWorkload => fmt.write_str("No workload with this name exists."),
            Self::GetStorage(get_storage) => Debug::fmt(get_storage, fmt),
        }
    }
}

impl Display for RunWorkload {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Error occuring when trying to sort a single packed storage.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    MissingPackStorage,
    TooManyStorages,
}

#[cfg(feature = "std")]
impl Error for Sort {}

impl Debug for Sort {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::MissingPackStorage => fmt.write_str("The storage you want to sort is packed, you may be able to sort the whole pack by passing all storages packed with it to the function. Some packs can't be sorted."),
            Self::TooManyStorages => fmt.write_str("You provided too many storages non packed together. Only single storage and storages packed together can be sorted."),
        }
    }
}

impl Display for Sort {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Register {
    Borrow(Borrow),
    WrongThread,
}

#[cfg(feature = "std")]
impl Error for Register {}

impl Debug for Register {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::Borrow(borrow) => Debug::fmt(borrow, fmt),
            Self::WrongThread => fmt.write_str(
                "Impossible to add !Send storages on other thread than World's (for now).",
            ),
        }
    }
}

impl Display for Register {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

impl From<Borrow> for Register {
    fn from(borrow: Borrow) -> Self {
        Self::Borrow(borrow)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NotUpdatePack;

#[cfg(feature = "std")]
impl Error for NotUpdatePack {}

impl Debug for NotUpdatePack {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        fmt.write_str("The storage isn't update packed. Use `view.update_pack()` to pack it.")
    }
}

impl Display for NotUpdatePack {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Inserted {
    NotUpdatePacked,
    NotInbound,
}

#[cfg(feature = "std")]
impl Error for Inserted {}

impl Debug for Inserted {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::NotUpdatePacked => fmt
                .write_str("The storage isn't update packed. Use `view.update_pack()` to pack it."),
            Self::NotInbound => {
                fmt.write_str("This window doesn't contain the inserted components.")
            }
        }
    }
}

impl Display for Inserted {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Modified {
    NotUpdatePacked,
    NotInbound,
}

#[cfg(feature = "std")]
impl Error for Modified {}

impl Debug for Modified {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::NotUpdatePacked => fmt
                .write_str("The storage isn't update packed. Use `view.update_pack()` to pack it."),
            Self::NotInbound => {
                fmt.write_str("This window doesn't contain the modified components.")
            }
        }
    }
}

impl Display for Modified {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InsertedOrModified {
    NotUpdatePacked,
    NotInbound,
}

#[cfg(feature = "std")]
impl Error for InsertedOrModified {}

impl Debug for InsertedOrModified {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::NotUpdatePacked => fmt
                .write_str("The storage isn't update packed. Use `view.update_pack()` to pack it."),
            Self::NotInbound => {
                fmt.write_str("This window doesn't contain the inserted or modified components.")
            }
        }
    }
}

impl Display for InsertedOrModified {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MissingComponent {
    pub id: EntityId,
    pub name: &'static str,
}

#[cfg(feature = "std")]
impl Error for MissingComponent {}

impl Debug for MissingComponent {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        fmt.write_fmt(format_args!(
            "{:?} doesn't have a {} component.",
            self.id, self.name
        ))
    }
}

impl Display for MissingComponent {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NotInbound {
    View(&'static str),
    Window,
    UpdatePack,
}

#[cfg(feature = "std")]
impl Error for NotInbound {}

impl Debug for NotInbound {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::View(name) => fmt.write_fmt(format_args!(
                "There isn't enough {} components to fill this range.",
                name,
            )),
            Self::Window => fmt.write_str("This window is too small to fill this range."),
            Self::UpdatePack => fmt.write_str("With update packed storages windows including *regular* components have to include the first non modified component.")
        }
    }
}

impl Display for NotInbound {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}
