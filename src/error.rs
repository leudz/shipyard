use crate::EntityId;
use alloc::boxed::Box;
use core::fmt::{Debug, Display, Formatter};
#[cfg(feature = "std")]
use std::error::Error;

/// AtomicRefCell's borrow error.
///
/// Unique means the BorrowState was mutably borrowed when an illegal borrow occured.
///
/// Shared means the BorrowState was immutably borrowed when an illegal borrow occured.
///
/// WrongThread is linked to !Send, when trying to access them from an other thread.
///
/// MultipleThreads is when !Send types are accessed from multiple threads.
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
    Entities(Borrow),
    MissingStorage(&'static str),
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
            Self::Entities(borrow) => match borrow {
                Borrow::Unique => fmt.write_str("Cannot mutably borrow Entities storage while it's already borrowed."),
                Borrow::Shared => {
                    fmt.write_str("Cannot immutably borrow Entities storage while it's already mutably borrowed.")
                },
                _ => unreachable!(),
            },
            Self::MissingStorage(name) => fmt.write_fmt(format_args!("{} storage was not found in the World. You can register unique storage with: world.add_unique(your_unique);", name)),
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AddWorkload {
    AlreadyExists,
    Borrow,
}

#[cfg(feature = "std")]
impl Error for AddWorkload {}

impl Debug for AddWorkload {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::AlreadyExists => fmt.write_str("A workload with this name already exists."),
            Self::Borrow => {
                fmt.write_str("Cannot mutably borrow the scheduler while it's already borrowed.")
            }
        }
    }
}

impl Display for AddWorkload {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Trying to set the default workload to a non existant one will result in this error.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SetDefaultWorkload {
    Borrow,
    MissingWorkload,
}

#[cfg(feature = "std")]
impl Error for SetDefaultWorkload {}

impl Debug for SetDefaultWorkload {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::Borrow => {
                fmt.write_str("Cannot mutably borrow scheduler while it's already borrowed.")
            }
            Self::MissingWorkload => fmt.write_str("No workload with this name exists."),
        }
    }
}

impl Display for SetDefaultWorkload {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Error related to `run_default` and `run_workload`.  
/// The error can be a storage error, problem with the scheduler's borrowing, a non existant workload or a custom error.
pub enum RunWorkload {
    Scheduler,
    Run((&'static str, Run)),
    MissingWorkload,
}

impl RunWorkload {
    #[cfg(feature = "std")]
    pub fn custom_error(self) -> Option<Box<dyn Error + Send>> {
        match self {
            Self::Run((_, Run::Custom(error))) => Some(error),
            _ => None,
        }
    }
    #[cfg(not(feature = "std"))]
    pub fn custom_error(self) -> Option<Box<dyn core::any::Any + Send>> {
        match self {
            Self::Run((_, Run::Custom(error))) => Some(error),
            _ => None,
        }
    }
}

#[cfg(feature = "std")]
impl Error for RunWorkload {}

impl Debug for RunWorkload {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::Scheduler => {
                fmt.write_str("Cannot borrow the scheduler while it's already mutably borrowed.")
            }
            Self::MissingWorkload => fmt.write_str("No workload with this name exists."),
            Self::Run((system_name, run)) => {
                fmt.write_fmt(format_args!("System {} failed: {:?}", system_name, run))
            }
        }
    }
}

impl Display for RunWorkload {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Error returned by `World::try_run` and `AllStorages::try_run`.  
/// Can refer to an invalid storage borrow or a custom error.
pub enum Run {
    GetStorage(GetStorage),
    #[cfg(feature = "std")]
    Custom(Box<dyn Error + Send>),
    #[cfg(not(feature = "std"))]
    Custom(Box<dyn core::any::Any + Send>),
}

impl From<GetStorage> for Run {
    fn from(get_storage: GetStorage) -> Self {
        Run::GetStorage(get_storage)
    }
}

impl Run {
    #[cfg(feature = "std")]
    pub fn from_custom<E: Error + Send + 'static>(error: E) -> Self {
        Run::Custom(Box::new(error))
    }
    #[cfg(not(feature = "std"))]
    pub fn from_custom<E: core::any::Any + Send>(error: E) -> Self {
        Run::Custom(Box::new(error))
    }
}

#[cfg(feature = "std")]
impl Error for Run {}

impl Debug for Run {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::GetStorage(get_storage) => Debug::fmt(&get_storage, fmt),
            Self::Custom(_) => fmt.write_fmt(format_args!("run failed with a custom error.")),
        }
    }
}

impl Display for Run {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Error occuring when trying to sort a packed storage but providing too few or too many storages.
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

/// Error when trying to use update pack related function on non update packed storage.
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

/// Error when trying to access the *inserted* section of an update packed storage but the storage isn't update packed or the section isn't present in the window.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UpdateWindow {
    NotUpdatePacked,
    OutOfBounds,
}

#[cfg(feature = "std")]
impl Error for UpdateWindow {}

impl Debug for UpdateWindow {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::NotUpdatePacked => fmt.write_str(
                "The storage isn't update packed. Use `view_mut.update_pack()` to pack it.",
            ),
            Self::OutOfBounds => {
                fmt.write_str("This window does not contain the components you want.")
            }
        }
    }
}

impl Display for UpdateWindow {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Error when using `get` with an entity that does not have any component in the requested storage(s).
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
            "{:?} does not have a {} component.",
            self.id, self.name
        ))
    }
}

impl Display for MissingComponent {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

/// Error related to window slicing, the range could be too big or trying to access an invalid range of an update packed window.
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

/// Trying to add an invalid system to a workload will return this error.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InvalidSystem {
    AllStorages,
    MultipleViews,
    MultipleViewsMut,
}

#[cfg(feature = "std")]
impl Error for InvalidSystem {}

impl Debug for InvalidSystem {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::AllStorages => fmt.write_str("A system borrowing both AllStorages and a storage can't run. You can borrow the storage inside the system with AllStorages::borrow or AllStorages::run instead."),
            Self::MultipleViews => fmt.write_str("Multiple views of the same storage including an exclusive borrow, consider removing the shared borrow."),
            Self::MultipleViewsMut => fmt.write_str("Multiple exclusive views of the same storage, consider removing one."),
        }
    }
}

impl Display for InvalidSystem {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum UniqueRemove {
    AllStorages,
    MissingUnique(&'static str),
    StorageBorrow((&'static str, Borrow)),
}

#[cfg(feature = "std")]
impl Error for UniqueRemove {}

impl Debug for UniqueRemove {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::AllStorages => fmt.write_str("Cannot borrow AllStorages while it's already exclusively borrowed."),
            Self::MissingUnique(name) => fmt.write_fmt(format_args!("No unique storage exists for {}.\n", name)),
            Self::StorageBorrow((name, borrow)) => match borrow {
                Borrow::Unique => fmt.write_fmt(format_args!("Cannot mutably borrow {} storage while it's already borrowed.", name)),
                Borrow::WrongThread => fmt.write_fmt(format_args!("Cannot borrow {} storage from other thread than the one it was created in because it's !Send and !Sync.", name)),
                _ => unreachable!()
            }
        }
    }
}

impl Display for UniqueRemove {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, fmt)
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Share;

#[cfg(feature = "std")]
impl Error for Share {}

impl Debug for Share {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        f.write_str("Cannot share the component of this entity because it already owns one.")
    }
}

impl Display for Share {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Unshare;

#[cfg(feature = "std")]
impl Error for Unshare {}

impl Debug for Unshare {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        f.write_str("Cannot unshare the component of this entity because it's not shared.")
    }
}

impl Display for Unshare {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Apply {
    IdenticalIds,
    MissingComponent(EntityId),
}

#[cfg(feature = "std")]
impl Error for Apply {}

impl Debug for Apply {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::IdenticalIds => f.write_str("Cannot use apply with identical components."),
            Self::MissingComponent(id) => f.write_fmt(format_args!(
                "Entity {:?} does not have any component in this storage.",
                id
            )),
        }
    }
}

impl Display for Apply {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}
