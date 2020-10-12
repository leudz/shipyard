use crate::add_entity::AddEntity;
use crate::atomic_refcell::{Ref, SharedBorrow};
use crate::borrow::AllStoragesBorrow;
#[cfg(feature = "non_send")]
use crate::borrow::NonSend;
#[cfg(all(feature = "non_send", feature = "non_sync"))]
use crate::borrow::NonSendSync;
#[cfg(feature = "non_sync")]
use crate::borrow::NonSync;
use crate::error;
use crate::storage::{AllStorages, EntityId};
use crate::view::{EntitiesViewMut, ViewMut};

/// Keeps information to create an entity.
pub struct EntityBuilder<'a, C, S> {
    components: C,
    storages: S,
    all_storages: &'a AllStorages,
    all_borrow: Option<SharedBorrow<'a>>,
}

impl Clone for EntityBuilder<'_, (), ()> {
    #[inline]
    fn clone(&self) -> Self {
        EntityBuilder {
            all_storages: self.all_storages,
            all_borrow: self.all_borrow.clone(),
            components: (),
            storages: (),
        }
    }
}

impl<'a> EntityBuilder<'a, (), ()> {
    #[inline]
    pub(crate) fn new(all_storages: Ref<'a, &'a AllStorages>) -> Self {
        let (all_storages, all_borrow) = unsafe { Ref::destructure(all_storages) };

        EntityBuilder {
            all_storages,
            all_borrow: Some(all_borrow),
            components: (),
            storages: (),
        }
    }

    #[inline]
    pub(crate) fn new_from_reference(all_storages: &'a AllStorages) -> Self {
        EntityBuilder {
            all_storages,
            all_borrow: None,
            components: (),
            storages: (),
        }
    }

    /// Adds a new component to the future entity.  
    /// Borrows the storage associated with it.
    #[inline]
    pub fn try_with<T: 'static + Send + Sync>(
        self,
        component: T,
    ) -> Result<EntityBuilder<'a, (T,), (ViewMut<'a, T>,)>, error::Borrow> {
        let storage = self.all_storages.try_borrow().map_err(|err| {
            if let error::GetStorage::StorageBorrow((_, borrow)) = err {
                borrow
            } else {
                unreachable!()
            }
        })?;

        Ok(EntityBuilder {
            all_storages: self.all_storages,
            all_borrow: self.all_borrow,
            components: (component,),
            storages: (storage,),
        })
    }

    /// Adds a new component to the future entity.  
    /// Borrows the storage associated with it panics if its already borrowed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    #[inline]
    pub fn with<T: 'static + Send + Sync>(
        self,
        component: T,
    ) -> EntityBuilder<'a, (T,), (ViewMut<'a, T>,)> {
        match self.all_storages.try_borrow() {
            Ok(storage) => EntityBuilder {
                all_storages: self.all_storages,
                all_borrow: self.all_borrow,
                components: (component,),
                storages: (storage,),
            },
            Err(err) => panic!("{:?}", err),
        }
    }

    /// Adds a new `!Send` component to the future entity.  
    /// Borrows the storage associated with it.
    #[cfg(feature = "non_send")]
    #[cfg_attr(docsrs, doc(cfg(feature = "non_send")))]
    #[inline]
    pub fn try_with_non_send<T: 'static + Sync>(
        self,
        component: T,
    ) -> Result<EntityBuilder<'a, (T,), (ViewMut<'a, T>,)>, error::Borrow> {
        let storage = self
            .all_storages
            .try_borrow::<NonSend<_>>()
            .map(|view_mut| view_mut.0)
            .map_err(|err| {
                if let error::GetStorage::StorageBorrow((_, borrow)) = err {
                    borrow
                } else {
                    unreachable!()
                }
            })?;

        Ok(EntityBuilder {
            all_storages: self.all_storages,
            all_borrow: self.all_borrow,
            components: (component,),
            storages: (storage,),
        })
    }

    /// Adds a new `!Send` component to the future entity.  
    /// Borrows the storage associated with it panics if its already borrowed.
    #[cfg(all(feature = "non_send", feature = "panic"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "non_send", feature = "panic"))))]
    #[track_caller]
    #[inline]
    pub fn with_non_send<T: 'static + Sync>(
        self,
        component: T,
    ) -> EntityBuilder<'a, (T,), (ViewMut<'a, T>,)> {
        match self
            .all_storages
            .try_borrow::<NonSend<_>>()
            .map(|view_mut| view_mut.0)
        {
            Ok(storage) => EntityBuilder {
                all_storages: self.all_storages,
                all_borrow: self.all_borrow,
                components: (component,),
                storages: (storage,),
            },
            Err(err) => panic!("{:?}", err),
        }
    }

    /// Adds a new `!Sync` component to the future entity.  
    /// Borrows the storage associated with it.
    #[cfg(feature = "non_sync")]
    #[cfg_attr(docsrs, doc(cfg(feature = "non_sync")))]
    #[inline]
    pub fn try_with_non_sync<T: 'static + Send>(
        self,
        component: T,
    ) -> Result<EntityBuilder<'a, (T,), (ViewMut<'a, T>,)>, error::Borrow> {
        let storage = self
            .all_storages
            .try_borrow::<NonSync<_>>()
            .map(|view_mut| view_mut.0)
            .map_err(|err| {
                if let error::GetStorage::StorageBorrow((_, borrow)) = err {
                    borrow
                } else {
                    unreachable!()
                }
            })?;

        Ok(EntityBuilder {
            all_storages: self.all_storages,
            all_borrow: self.all_borrow,
            components: (component,),
            storages: (storage,),
        })
    }

    /// Adds a new `!Sync` component to the future entity.  
    /// Borrows the storage associated with it panics if its already borrowed.
    #[cfg(all(feature = "non_sync", feature = "panic"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "non_sync", feature = "panic"))))]
    #[track_caller]
    #[inline]
    pub fn with_non_sync<T: 'static + Send>(
        self,
        component: T,
    ) -> EntityBuilder<'a, (T,), (ViewMut<'a, T>,)> {
        match self
            .all_storages
            .try_borrow::<NonSync<_>>()
            .map(|view_mut| view_mut.0)
        {
            Ok(storage) => EntityBuilder {
                all_storages: self.all_storages,
                all_borrow: self.all_borrow,
                components: (component,),
                storages: (storage,),
            },
            Err(err) => panic!("{:?}", err),
        }
    }

    /// Adds a new `!Send + !Sync` component to the future entity.  
    /// Borrows the storage associated with it.
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "non_send", feature = "non_sync"))))]
    #[inline]
    pub fn try_with_non_send_sync<T: 'static>(
        self,
        component: T,
    ) -> Result<EntityBuilder<'a, (T,), (ViewMut<'a, T>,)>, error::Borrow> {
        let storage = self
            .all_storages
            .try_borrow::<NonSendSync<_>>()
            .map(|view_mut| view_mut.0)
            .map_err(|err| {
                if let error::GetStorage::StorageBorrow((_, borrow)) = err {
                    borrow
                } else {
                    unreachable!()
                }
            })?;

        Ok(EntityBuilder {
            all_storages: self.all_storages,
            all_borrow: self.all_borrow,
            components: (component,),
            storages: (storage,),
        })
    }

    /// Adds a new `!Send + !Sync` component to the future entity.  
    /// Borrows the storage associated with it, panics if its already borrowed.
    #[cfg(all(feature = "non_send", feature = "non_sync", feature = "panic"))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(feature = "non_send", feature = "non_sync", feature = "panic")))
    )]
    #[track_caller]
    #[inline]
    pub fn with_non_send_sync<T: 'static>(
        self,
        component: T,
    ) -> EntityBuilder<'a, (T,), (ViewMut<'a, T>,)> {
        match self
            .all_storages
            .try_borrow::<NonSendSync<_>>()
            .map(|view_mut| view_mut.0)
        {
            Ok(storage) => EntityBuilder {
                all_storages: self.all_storages,
                all_borrow: self.all_borrow,
                components: (component,),
                storages: (storage,),
            },
            Err(err) => panic!("{:?}", err),
        }
    }

    /// Adds the entity to the `World`.
    ///
    /// Borrows the `Entities` storage.
    #[inline]
    pub fn try_build(self) -> Result<EntityId, error::GetStorage> {
        Ok(self
            .all_storages
            .try_borrow::<EntitiesViewMut<'_>>()?
            .add_entity((), ()))
    }

    /// Adds the entity to the `World`.
    ///
    /// Borrows the `Entities` storage, panics if its already borrowed.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    #[inline]
    pub fn build(self) -> EntityId {
        match self.try_build() {
            Ok(id) => id,
            Err(err) => panic!("{:?}", err),
        }
    }
}

macro_rules! impl_entity_builder {
    ($(($type: ident, $storage_type: ident, $index: tt))+) => {
        impl<'a, $($type: 'static,)+ $($storage_type),+> EntityBuilder<'a, ($($type,)+), ($($storage_type,)+)> {
            /// Adds a new component to the future entity.
            ///
            /// Borrows the storage associated with it.
            #[inline]
            pub fn try_with<T: 'static + Send + Sync>(self, component: T) -> Result<EntityBuilder<'a, ($($type,)+ T,), ($($storage_type,)+ ViewMut<'a, T>,)>, error::Borrow> {
                let storage = AllStoragesBorrow::try_borrow(self.all_storages).map_err(|err| {
                    if let error::GetStorage::StorageBorrow((_, borrow)) = err {
                        borrow
                    } else {
                        unreachable!()
                    }
                })?;

                Ok(EntityBuilder {
                    all_storages: self.all_storages,
                    all_borrow: self.all_borrow,
                    components: ($(self.components.$index,)+ component,),
                    storages: ($(self.storages.$index,)+ storage,),
                })
            }

            /// Adds a new component to the future entity.
            ///
            /// Borrows the storage associated with it panics if its already borrowed.
            #[cfg(feature = "panic")]
            #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
            #[track_caller]
            #[inline]
            pub fn with<T: 'static + Send + Sync>(self, component: T) -> EntityBuilder<'a, ($($type,)+ T,), ($($storage_type,)+ ViewMut<'a, T>,)> {
                match self.all_storages.try_borrow() {
                    Ok(storage) => EntityBuilder {
                        all_storages: self.all_storages,
                        all_borrow: self.all_borrow,
                        components: ($(self.components.$index,)+ component,),
                        storages: ($(self.storages.$index,)+ storage,),
                    },
                    Err(err) => panic!("{:?}", err),
                }
            }

            /// Adds a new `!Send` component to the future entity.
            ///
            /// Borrows the storage associated with it.
            #[cfg(feature = "non_send")]
            #[cfg_attr(docsrs, doc(cfg(feature = "non_send")))]
            #[inline]
            pub fn try_with_non_send<T: 'static + Sync>(self, component: T) -> Result<EntityBuilder<'a, ($($type,)+ T,), ($($storage_type,)+ ViewMut<'a, T>,)>, error::Borrow> {
                let storage = self
                    .all_storages
                    .try_borrow::<NonSend<_>>()
                    .map(|view_mut| view_mut.0)
                    .map_err(|err| {
                        if let error::GetStorage::StorageBorrow((_, borrow)) = err {
                            borrow
                        } else {
                            unreachable!()
                        }
                    })?;

                Ok(EntityBuilder {
                    all_storages: self.all_storages,
                    all_borrow: self.all_borrow,
                    components: ($(self.components.$index,)+ component,),
                    storages: ($(self.storages.$index,)+ storage,),
                })
            }

            /// Adds a new `!Send` component to the future entity.
            ///
            /// Borrows the storage associated with it panics if its already borrowed.
            #[cfg(all(feature = "non_send", feature = "panic"))]
            #[cfg_attr(docsrs, doc(cfg(all(feature = "non_send", feature = "panic"))))]
            #[track_caller]
            #[inline]
            pub fn with_non_send<T: 'static + Sync>(self, component: T) -> EntityBuilder<'a, ($($type,)+ T,), ($($storage_type,)+ ViewMut<'a, T>,)> {
                match self
                    .all_storages
                    .try_borrow::<NonSend<_>>()
                    .map(|view_mut| view_mut.0)
                {
                    Ok(storage) => EntityBuilder {
                        all_storages: self.all_storages,
                        all_borrow: self.all_borrow,
                        components: ($(self.components.$index,)+ component,),
                        storages: ($(self.storages.$index,)+ storage,),
                    },
                    Err(err) => panic!("{:?}", err),
                }
            }

            /// Adds a new `!Sync` component to the future entity.
            ///
            /// Borrows the storage associated with it.
            #[cfg(feature = "non_sync")]
            #[cfg_attr(docsrs, doc(cfg(feature = "non_sync")))]
            #[inline]
            pub fn try_with_non_sync<T: 'static + Send>(self, component: T) -> Result<EntityBuilder<'a, ($($type,)+ T,), ($($storage_type,)+ ViewMut<'a, T>,)>, error::Borrow> {
                let storage = self
                    .all_storages
                    .try_borrow::<NonSync<_>>()
                    .map(|view_mut| view_mut.0)
                    .map_err(|err| {
                        if let error::GetStorage::StorageBorrow((_, borrow)) = err {
                            borrow
                        } else {
                            unreachable!()
                        }
                    })?;

                Ok(EntityBuilder {
                    all_storages: self.all_storages,
                    all_borrow: self.all_borrow,
                    components: ($(self.components.$index,)+ component,),
                    storages: ($(self.storages.$index,)+ storage,),
                })
            }

            /// Adds a new `!Sync` component to the future entity.
            ///
            /// Borrows the storage associated with it panics if its already borrowed.
            #[cfg(all(feature = "non_sync", feature = "panic"))]
            #[cfg_attr(docsrs, doc(cfg(all(feature = "non_sync", feature = "panic"))))]
            #[track_caller]
            #[inline]
            pub fn with_non_sync<T: 'static + Send>(self, component: T) -> EntityBuilder<'a, ($($type,)+ T,), ($($storage_type,)+ ViewMut<'a, T>,)> {
                match self
                    .all_storages
                    .try_borrow::<NonSync<_>>()
                    .map(|view_mut| view_mut.0)
                {
                    Ok(storage) => EntityBuilder {
                        all_storages: self.all_storages,
                        all_borrow: self.all_borrow,
                        components: ($(self.components.$index,)+ component,),
                        storages: ($(self.storages.$index,)+ storage,),
                    },
                    Err(err) => panic!("{:?}", err),
                }
            }

            /// Adds a new `!Send + !Sync` component to the future entity.
            ///
            /// Borrows the storage associated with it.
            #[cfg(all(feature = "non_send", feature = "non_sync"))]
            #[cfg_attr(docsrs, doc(cfg(all(feature = "non_send", feature = "non_sync"))))]
            #[inline]
            pub fn try_with_non_send_sync<T: 'static>(self, component: T) -> Result<EntityBuilder<'a, ($($type,)+ T,), ($($storage_type,)+ ViewMut<'a, T>,)>, error::Borrow> {
                let storage = self
                    .all_storages
                    .try_borrow::<NonSendSync<_>>()
                    .map(|view_mut| view_mut.0)
                    .map_err(|err| {
                        if let error::GetStorage::StorageBorrow((_, borrow)) = err {
                            borrow
                        } else {
                            unreachable!()
                        }
                    })?;

                Ok(EntityBuilder {
                    all_storages: self.all_storages,
                    all_borrow: self.all_borrow,
                    components: ($(self.components.$index,)+ component,),
                    storages: ($(self.storages.$index,)+ storage,),
                })
            }

            /// Adds a new `!Send + !Sync` component to the future entity.
            ///
            /// Borrows the storage associated with it, panics if its already borrowed.
            #[cfg(all(feature = "non_send", feature = "non_sync", feature = "panic"))]
            #[cfg_attr(docsrs, doc(cfg(all(feature = "non_send", feature = "non_sync", feature = "panic"))))]
            #[track_caller]
            #[inline]
            pub fn with_non_send_sync<T: 'static>(self, component: T) -> EntityBuilder<'a, ($($type,)+ T,), ($($storage_type,)+ ViewMut<'a, T>,)> {
                match self
                    .all_storages
                    .try_borrow::<NonSendSync<_>>()
                    .map(|view_mut| view_mut.0)
                {
                    Ok(storage) => EntityBuilder {
                        all_storages: self.all_storages,
                        all_borrow: self.all_borrow,
                        components: ($(self.components.$index,)+ component,),
                        storages: ($(self.storages.$index,)+ storage,),
                    },
                    Err(err) => panic!("{:?}", err),
                }
            }

            /// Adds the entity to the `World`.
            ///
            /// Borrows the `Entities` storage.
            #[inline]
            pub fn try_build(self) -> Result<EntityId, error::GetStorage> where ($($storage_type,)+): AddEntity<Component = ($($type,)+)> {
                Ok(self
                    .all_storages
                    .try_borrow::<EntitiesViewMut<'_>>()?
                    .add_entity(self.storages, self.components))
            }

            /// Adds the entity to the `World`.
            ///
            /// Borrows the `Entities` storage, panics if its already borrowed.
            #[cfg(feature = "panic")]
            #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
            #[track_caller]
            #[inline]
            pub fn build(self) -> EntityId where ($($storage_type,)+): AddEntity<Component = ($($type,)+)> {
                match self.try_build() {
                    Ok(id) => id,
                    Err(err) => panic!("{:?}", err),
                }
            }
        }
    }
}

macro_rules! entity_builder {
    ($(($type: ident, $storage_type: ident, $index: tt))+; ($type1: ident, $storage_type1: ident, $index1: tt) $(($queue_type: ident, $queue_storage_type: ident, $queue_index: tt))*) => {
        impl_entity_builder![$(($type, $storage_type, $index))*];
        entity_builder![$(($type, $storage_type, $index))* ($type1, $storage_type1, $index1); $(($queue_type, $queue_storage_type, $queue_index))*];
    };
    ($(($type: ident, $storage_type: ident, $index: tt))+;) => {
        impl<'a, $($type: 'static,)+ $($storage_type),+> EntityBuilder<'a, ($($type,)+), ($($storage_type,)+)> {
            #[inline]
            pub fn try_build(self) -> Result<EntityId, error::GetStorage> where ($($storage_type,)+): AddEntity<Component = ($($type,)+)> {
                Ok(self
                    .all_storages
                    .try_borrow::<EntitiesViewMut<'_>>()?
                    .add_entity(self.storages, self.components))
            }

            #[cfg(feature = "panic")]
            #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
            #[track_caller]
            #[inline]
            pub fn build(self) -> EntityId where ($($storage_type,)+): AddEntity<Component = ($($type,)+)> {
                match self.try_build() {
                    Ok(id) => id,
                    Err(err) => panic!("{:?}", err),
                }
            }
        }
    }
}

entity_builder![(A, AS, 0); (B, BS, 1) (C, CS, 2) (D, DS, 3) (E, ES, 4) (F, FS, 5) (G, GS, 6) (H, HS, 7) (I, IS, 8) (J, JS, 9)];
