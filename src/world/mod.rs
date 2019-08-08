mod new_entity;
mod pack;
mod register;

use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::component_storage::AllStorages;
use crate::entity::{Entities, Key};
use crate::error;
use crate::get_storage::GetStorage;
use crate::pack::OwnedPack;
use crate::run::Run;
use new_entity::WorldNewEntity;
use pack::WorldOwnedPack;
use register::Register;

/// `World` holds all components and keeps track of entities and what they own.
pub struct World {
    entities: AtomicRefCell<Entities>,
    storages: AtomicRefCell<AllStorages>,
}

impl Default for World {
    /// Create an empty `World` without any storage.
    fn default() -> Self {
        World {
            entities: AtomicRefCell::new(Default::default()),
            storages: AtomicRefCell::new(Default::default()),
        }
    }
}

impl World {
    /// Create a `World` with storages based on `T`.
    ///
    /// `T` has to be a tuple even for a single type due to restrictions.
    /// In this case use (T,).
    pub fn new<T: Register>() -> Self {
        let world = World::default();
        T::register(&world);
        world
    }
    /// Same as `try_get_storage` but will `unwrap` any error.
    pub fn get_storage<'a, T: GetStorage<'a>>(&'a self) -> T::Storage {
        self.try_get_storage::<T>().unwrap()
    }
    /// Retrives storages based on type `T`.
    /// `&T` returns a read access to the storage.
    /// `&mut T` returns a write access to the storage.
    /// To retrive multiple storages at once, use a tuple.
    ///
    /// `T` has to be a tuple even for a single type due to restrictions.
    /// In this case use (T,).
    pub fn try_get_storage<'a, T: GetStorage<'a>>(
        &'a self,
    ) -> Result<T::Storage, error::GetStorage> {
        Ok(self
            .try_all_storages()
            .map_err(error::GetStorage::AllStoragesBorrow)?
            .try_get_storage::<T>()?)
    }
    /// Same as `try_new_entity` but will `unwrap` any error.
    pub fn new_entity<T: WorldNewEntity>(&self, component: T) -> Key {
        self.try_new_entity::<T>(component).unwrap()
    }
    /// Stores `component` in a new entity, the `Key` to this entity is returned.
    /// As opposed to `add_entity`, storages will be created if they don't exist.
    /// Multiple components can be added at the same time using a tuple.
    ///
    /// `T` has to be a tuple even for a single type due to restrictions.
    /// In this case use (T,).
    pub fn try_new_entity<T: WorldNewEntity>(&self, component: T) -> Result<Key, error::NewEntity> {
        let mut entities = self
            .try_entities_mut()
            .map_err(error::NewEntity::Entities)?;
        let mut storages = self
            .storages
            .try_borrow_mut()
            .map_err(error::NewEntity::AllStoragesBorrow)?;
        Ok(T::new_entity(component, &mut *storages, &mut *entities))
    }
    /// Same as `try_entities_mut` but will `unwrap` any error.
    pub fn entities_mut(&self) -> RefMut<Entities> {
        self.try_entities_mut().unwrap()
    }
    /// Returns a mutable reference to the entities' storage.
    pub fn try_entities_mut(&self) -> Result<RefMut<Entities>, error::Borrow> {
        Ok(self.entities.try_borrow_mut()?)
    }
    /// Same as `try_all_storages` but will `unwrap` any error.
    pub fn all_storages(&self) -> Ref<AllStorages> {
        self.try_all_storages().unwrap()
    }
    /// Returns an immutable reference to the storage of all storages.
    pub fn try_all_storages(&self) -> Result<Ref<AllStorages>, error::Borrow> {
        Ok(self.storages.try_borrow()?)
    }
    /// Same as `try_all_storages` but will `unwrap` any error.
    pub fn all_storages_mut(&self) -> RefMut<AllStorages> {
        self.try_all_storages_mut().unwrap()
    }
    /// Returns an immutable reference to the storage of all storages.
    pub fn try_all_storages_mut(&self) -> Result<RefMut<AllStorages>, error::Borrow> {
        Ok(self.storages.try_borrow_mut()?)
    }
    /// Same as `try_register` but will `unwrap` any error.
    pub fn register<T: 'static + Send + Sync>(&self) {
        self.try_register::<T>().unwrap()
    }
    /// Register a new component type and create a storage for it.
    /// Does nothing if a storage already exists.
    pub fn try_register<T: 'static + Send + Sync>(&self) -> Result<(), error::Borrow> {
        self.storages.try_borrow_mut()?.register::<T>();
        Ok(())
    }
    /// Allows to perform some actions not possible otherwise like iteration.
    /// Each type has to come with a mutablility expressed by `&` or `&mut`.
    /// `Entities` are the exception, they only come in mutable flavor.
    /// Multiple types can be queried by using a tuple.
    ///
    /// `T` has to be a tuple even for a single type due to restrictions.
    /// In this case use (T,).
    pub fn run<'a, T: Run<'a>, F: FnOnce(T::Storage)>(&'a self, f: F) {
        T::run(&self.entities, &self.storages, f);
    }
    /// Pack multiple storages, it can speed up iteration at the cost of insertion/removal.
    pub fn try_pack_owned<'a, T: WorldOwnedPack<'a>>(&'a self) -> Result<(), error::WorldPack>
    where
        <T as WorldOwnedPack<'a>>::Storage: GetStorage<'a>,
        <<T as WorldOwnedPack<'a>>::Storage as GetStorage<'a>>::Storage: OwnedPack,
    {
        self.try_get_storage::<T::Storage>()?.try_pack_owned()?;
        Ok(())
    }
    /// Same as `try_pack_owned` but will unwrap in case of error.
    pub fn pack_owned<'a, T: WorldOwnedPack<'a>>(&'a self)
    where
        <T as WorldOwnedPack<'a>>::Storage: GetStorage<'a>,
        <<T as WorldOwnedPack<'a>>::Storage as GetStorage<'a>>::Storage: OwnedPack,
    {
        self.try_pack_owned::<T>().unwrap()
    }
    /// Delete an entity and all its components.
    /// Returns true if the entity was alive.
    pub fn try_delete(&self, entity: Key) -> Result<bool, error::Borrow> {
        let mut entities = self.try_entities_mut()?;
        let storages = self.try_all_storages_mut()?;
        Ok(entities.delete(storages, entity))
    }
    /// Same as try_delete but will unwrap any error.
    pub fn delete(&self, entity: Key) -> bool {
        self.try_delete(entity).unwrap()
    }
}
