//! All error types.

use crate::info::TypeInfo;
use crate::storage::StorageId;
use crate::EntityId;
use alloc::borrow::Cow;
use alloc::boxed::Box;
use core::fmt::{Debug, Display, Formatter};
#[cfg(feature = "std")]
use std::error::Error;

/// AtomicRefCell's borrow error.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Borrow {
    /// The Storage was borrowed when an exclusive borrow occured.
    Unique,
    /// The Storage was borrowed exclusively when a shared borrow occured.
    Shared,
    /// The Storage of a `!Send` component was accessed from an other thread.
    WrongThread,
    /// The Storage of a `!Sync` component was accessed from multiple threads at the same time.
    MultipleThreads,
}

#[cfg(feature = "std")]
impl Error for Borrow {}

impl Debug for Borrow {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::Unique => f.write_str("Cannot mutably borrow while already borrowed."),
            Self::Shared => f.write_str("Cannot immutably borrow while already mutably borrowed."),
            Self::WrongThread => {
                f.write_str("Can't access from another thread because it's !Send and !Sync.")
            }
            Self::MultipleThreads => f.write_str(
                "Can't access from multiple threads at the same time because it's !Sync.",
            ),
        }
    }
}

impl Display for Borrow {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}

/// Error related to acquiring a storage.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GetStorage {
    #[allow(missing_docs)]
    AllStoragesBorrow(Borrow),
    #[allow(missing_docs)]
    StorageBorrow {
        name: Option<&'static str>,
        id: StorageId,
        borrow: Borrow,
    },
    #[allow(missing_docs)]
    Entities(Borrow),
    #[allow(missing_docs)]
    MissingStorage {
        name: Option<&'static str>,
        id: StorageId,
    },
}

#[cfg(feature = "std")]
impl Error for GetStorage {}

impl Debug for GetStorage {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::AllStoragesBorrow(borrow) => match borrow {
                Borrow::Unique => f.write_str("Cannot mutably borrow AllStorages while it's already borrowed (AllStorages is borrowed to access any storage)."),
                Borrow::Shared => {
                    f.write_str("Cannot immutably borrow AllStorages while it's already mutably borrowed.")
                },
                _ => unreachable!(),
            },
            Self::StorageBorrow {name, id, borrow} => if let Some(name) = name {
                match borrow {
                    Borrow::Unique => f.write_fmt(format_args!("Cannot mutably borrow {} storage while it's already borrowed.", name)),
                    Borrow::Shared => {
                        f.write_fmt(format_args!("Cannot immutably borrow {} storage while it's already mutably borrowed.", name))
                    },
                    Borrow::MultipleThreads => f.write_fmt(format_args!("Cannot borrow {} storage from multiple thread at the same time because it's !Sync.", name)),
                    Borrow::WrongThread => f.write_fmt(format_args!("Cannot borrow {} storage from other thread than the one it was created in because it's !Send and !Sync.", name)),
                }
            } else {
                match borrow {
                    Borrow::Unique => f.write_fmt(format_args!("Cannot mutably borrow {:?} storage while it's already borrowed.", id)),
                    Borrow::Shared => {
                        f.write_fmt(format_args!("Cannot immutably borrow {:?} storage while it's already mutably borrowed.", id))
                    },
                    Borrow::MultipleThreads => f.write_fmt(format_args!("Cannot borrow {:?} storage from multiple thread at the same time because it's !Sync.", id)),
                    Borrow::WrongThread => f.write_fmt(format_args!("Cannot borrow {:?} storage from other thread than the one it was created in because it's !Send and !Sync.", id)),
                }
            }
            Self::Entities(borrow) => match borrow {
                Borrow::Unique => f.write_str("Cannot mutably borrow Entities storage while it's already borrowed."),
                Borrow::Shared => {
                    f.write_str("Cannot immutably borrow Entities storage while it's already mutably borrowed.")
                },
                _ => unreachable!(),
            },
            Self::MissingStorage { name, id } => if let Some(name) = name {
                    f.write_fmt(format_args!("{} storage was not found in the World. You can register unique storage with: world.add_unique(your_unique);", name))
                } else {
                    f.write_fmt(format_args!("{:?} storage was not found in the World. You can register unique storage with: world.add_unique(your_unique);", id))
                }
        }
    }
}

impl Display for GetStorage {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}

/// Error related to adding an entity.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NewEntity {
    /// Another add_storage operation is in progress.
    AllStoragesBorrow(Borrow),
    /// Entities is already borrowed.
    Entities(Borrow),
}

#[cfg(feature = "std")]
impl Error for NewEntity {}

impl Debug for NewEntity {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::AllStoragesBorrow(borrow) => match borrow {
                Borrow::Unique => f.write_str("Cannot mutably borrow all storages while it's already borrowed (this include component storage)."),
                Borrow::Shared => {
                    f.write_str("Cannot immutably borrow all storages while it's already mutably borrowed.")
                },
                _ => unreachable!(),
            },
            Self::Entities(borrow) => match borrow {
                Borrow::Unique => f.write_str("Cannot mutably borrow entities while it's already borrowed."),
                _ => unreachable!(),
            },
        }
    }
}

impl Display for NewEntity {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}

/// Retured by [`AllStorages::add_component`] and [`World::add_component`] when trying to add components to an entity that is not alive.
///
/// [`AllStorages::add_component`]: crate::all_storages::AllStorages::add_component()
/// [`World::add_component`]: crate::world::World::add_component()
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AddComponent {
    #[allow(missing_docs)]
    EntityIsNotAlive,
}

#[cfg(feature = "std")]
impl Error for AddComponent {}

impl Debug for AddComponent {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::EntityIsNotAlive => f.write_str("Entity has to be alive to add component to it."),
        }
    }
}

impl Display for AddComponent {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}

/// Error type returned by [`WorkloadBuilder::add_to_world`].
///
/// [`WorkloadBuilder::add_to_world`]: crate::WorkloadBuilder::add_to_world()
#[derive(Clone, PartialEq, Eq)]
pub enum AddWorkload {
    /// A workload with the same name already exists.
    AlreadyExists,
    /// The `Scheduler` is already borrowed.
    Borrow,
    /// Unknown nested workload.
    UnknownWorkload(Cow<'static, str>, Cow<'static, str>),
}

#[cfg(feature = "std")]
impl Error for AddWorkload {}

impl Debug for AddWorkload {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::AlreadyExists => f.write_str("A workload with this name already exists."),
            Self::Borrow => {
                f.write_str("Cannot mutably borrow the scheduler while it's already borrowed.")
            }
            Self::UnknownWorkload(workload, unknown_workload) => f.write_fmt(format_args!(
                "Could not find {} workload while building {}'s batches.",
                unknown_workload, workload
            )),
        }
    }
}

impl Display for AddWorkload {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}

/// Trying to set the default workload to a non existant one will result in this error.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SetDefaultWorkload {
    /// The `Scheduler` is already borrowed.
    Borrow,
    /// The workload does not exists.
    MissingWorkload,
}

#[cfg(feature = "std")]
impl Error for SetDefaultWorkload {}

impl Debug for SetDefaultWorkload {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::Borrow => {
                f.write_str("Cannot mutably borrow scheduler while it's already borrowed.")
            }
            Self::MissingWorkload => f.write_str("No workload with this name exists."),
        }
    }
}

impl Display for SetDefaultWorkload {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}

/// Error returned by [`run_default`] and [`run_workload`].  
/// The error can be a storage error, problem with the scheduler's borrowing, a non existant workload or a custom error.
///
/// [`run_default`]: crate::World#method::run_default()
/// [`run_workload`]: crate::World#method::run_workload()
pub enum RunWorkload {
    /// The `Scheduler` is exclusively borrowed.
    Scheduler,
    /// Error while running a system.
    Run((&'static str, Run)),
    /// Workload is not present in the world.
    MissingWorkload,
}

impl RunWorkload {
    /// Helper function to get back a custom error.
    #[cfg(feature = "std")]
    pub fn custom_error(self) -> Option<Box<dyn Error + Send + Sync>> {
        match self {
            Self::Run((_, Run::Custom(error))) => Some(error),
            _ => None,
        }
    }
    /// Helper function to get back a custom error.
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
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::Scheduler => {
                f.write_str("Cannot borrow the scheduler while it's already mutably borrowed.")
            }
            Self::MissingWorkload => f.write_str("No workload with this name exists."),
            Self::Run((system_name, run)) => {
                f.write_fmt(format_args!("System {} failed: {:?}", system_name, run))
            }
        }
    }
}

impl Display for RunWorkload {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}

/// Error returned by [`World::run`] and [`AllStorages::run`].  
/// Can refer to an invalid storage borrow or a custom error.
///
/// [`World::run`]: crate::World::run()
/// [`AllStorages::run`]: crate::AllStorages::run()
pub enum Run {
    /// Failed to borrow one of the storage.
    GetStorage(GetStorage),
    /// Error returned by the system.
    #[cfg(feature = "std")]
    Custom(Box<dyn Error + Send + Sync>),
    /// Error returned by the system.
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
    pub(crate) fn from_custom<E: Into<Box<dyn Error + Send + Sync>>>(error: E) -> Self {
        Run::Custom(error.into())
    }
    #[cfg(not(feature = "std"))]
    pub(crate) fn from_custom<E: core::any::Any + Send>(error: E) -> Self {
        Run::Custom(Box::new(error))
    }
}

#[cfg(feature = "std")]
impl Error for Run {}

impl Debug for Run {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::GetStorage(get_storage) => Debug::fmt(&get_storage, f),
            Self::Custom(err) => {
                f.write_fmt(format_args!("run failed with a custom error, {:?}.", err))
            }
        }
    }
}

impl Display for Run {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}

/// Returned by [`get`] when an entity does not have a component in the requested storage(s).
///
/// [`get`]: crate::Get
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MissingComponent {
    /// `EntityId` of the component.
    pub id: EntityId,
    /// Name of the component.
    pub name: &'static str,
}

#[cfg(feature = "std")]
impl Error for MissingComponent {}

impl Debug for MissingComponent {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        f.write_fmt(format_args!(
            "{:?} does not have a {} component.",
            self.id, self.name
        ))
    }
}

impl Display for MissingComponent {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}

/// Returned when trying to add an invalid system to a workload.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InvalidSystem {
    /// `AllStorages` borrowed alongside another storage.
    AllStorages,
    /// Multiple views of the same storage including an exclusive one.
    MultipleViews,
    /// Multiple exclusive views fo the same storage.
    MultipleViewsMut,
}

#[cfg(feature = "std")]
impl Error for InvalidSystem {}

impl Debug for InvalidSystem {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::AllStorages => f.write_str("A system borrowing both AllStorages and a storage can't run. You can borrow the storage inside the system with AllStorages::borrow or AllStorages::run instead."),
            Self::MultipleViews => f.write_str("Multiple views of the same storage including an exclusive borrow, consider removing the shared borrow."),
            Self::MultipleViewsMut => f.write_str("Multiple exclusive views of the same storage, consider removing one."),
        }
    }
}

impl Display for InvalidSystem {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}

/// Error returned by [`World::remove_unique`] and [`AllStorages::remove_unique`].
///
/// [`World::remove_unique`]: crate::World::remove_unique()
/// [`AllStorages::remove_unique`]: crate::AllStorages::remove_unique()
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum UniqueRemove {
    /// `AllStorages` was already borrowed.
    AllStorages,
    /// No unique storage of this type exist.
    MissingUnique(&'static str),
    /// The uniuqe storage is already borrowed.
    StorageBorrow((&'static str, Borrow)),
}

#[cfg(feature = "std")]
impl Error for UniqueRemove {}

impl Debug for UniqueRemove {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Self::AllStorages => f.write_str("Cannot borrow AllStorages while it's already exclusively borrowed."),
            Self::MissingUnique(name) => f.write_fmt(format_args!("No unique storage exists for {}.\n", name)),
            Self::StorageBorrow((name, borrow)) => match borrow {
                Borrow::Unique => f.write_fmt(format_args!("Cannot mutably borrow {} storage while it's already borrowed.", name)),
                Borrow::WrongThread => f.write_fmt(format_args!("Cannot borrow {} storage from other thread than the one it was created in because it's !Send and !Sync.", name)),
                _ => unreachable!()
            }
        }
    }
}

impl Display for UniqueRemove {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}

/// Error returned by [`apply`] and [`apply_mut`].
///
/// [`apply`]: crate::SparseSet::apply()
/// [`apply_mut`]: crate::SparseSet::apply_mut()
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Apply {
    #[allow(missing_docs)]
    IdenticalIds,
    /// Entity that doesn't have the required component.
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

/// Error returned by [`are_all_uniques_present_in_world`].
///
/// [`are_all_uniques_present_in_world`]: crate::WorkloadBuilder::are_all_uniques_present_in_world()
#[derive(Clone, PartialEq, Eq)]
pub enum UniquePresence {
    #[allow(missing_docs)]
    Workload(Cow<'static, str>),
    #[allow(missing_docs)]
    Unique(TypeInfo),
}

#[cfg(feature = "std")]
impl Error for UniquePresence {}

impl Debug for UniquePresence {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            UniquePresence::Workload(workload) => f.write_fmt(format_args!(
                "{} workload is not present in the World.",
                workload
            )),
            UniquePresence::Unique(type_info) => f.write_fmt(format_args!(
                "{} unique storage is not present in the World",
                type_info.name
            )),
        }
    }
}

impl Display for UniquePresence {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        Debug::fmt(self, f)
    }
}
