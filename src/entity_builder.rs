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
use core::marker::PhantomData;

/// Keeps information to create an entity.
pub struct EntityBuilder<'a, C, S> {
    components: C,
    storages: PhantomData<S>,
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
            storages: PhantomData,
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
            storages: PhantomData,
        }
    }

    #[inline]
    pub(crate) fn new_from_reference(all_storages: &'a AllStorages) -> Self {
        EntityBuilder {
            all_storages,
            all_borrow: None,
            components: (),
            storages: PhantomData,
        }
    }

    /// Adds a new component to the future entity.
    #[inline]
    pub fn with<T: 'static + Send + Sync>(
        self,
        component: T,
    ) -> EntityBuilder<'a, (T,), (ViewMut<'a, T>,)> {
        EntityBuilder {
            all_storages: self.all_storages,
            all_borrow: self.all_borrow,
            components: (component,),
            storages: PhantomData,
        }
    }

    /// Adds a new `!Send` component to the future entity.
    #[cfg(feature = "non_send")]
    #[cfg_attr(docsrs, doc(cfg(feature = "non_send")))]
    #[inline]
    pub fn with_non_send<T: 'static + Sync>(
        self,
        component: T,
    ) -> EntityBuilder<'a, (T,), (NonSend<ViewMut<'a, T>>,)> {
        EntityBuilder {
            all_storages: self.all_storages,
            all_borrow: self.all_borrow,
            components: (component,),
            storages: PhantomData,
        }
    }

    /// Adds a new `!Sync` component to the future entity.
    #[cfg(feature = "non_sync")]
    #[cfg_attr(docsrs, doc(cfg(feature = "non_sync")))]
    #[inline]
    pub fn with_non_sync<T: 'static + Send>(
        self,
        component: T,
    ) -> EntityBuilder<'a, (T,), (NonSync<ViewMut<'a, T>>,)> {
        EntityBuilder {
            all_storages: self.all_storages,
            all_borrow: self.all_borrow,
            components: (component,),
            storages: PhantomData,
        }
    }

    /// Adds a new `!Send + !Sync` component to the future entity.
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "non_send", feature = "non_sync"))))]
    #[inline]
    pub fn with_non_send_sync<T: 'static>(
        self,
        component: T,
    ) -> EntityBuilder<'a, (T,), (NonSendSync<ViewMut<'a, T>>,)> {
        EntityBuilder {
            all_storages: self.all_storages,
            all_borrow: self.all_borrow,
            components: (component,),
            storages: PhantomData,
        }
    }

    /// Adds the entity to the `World`.
    ///
    /// ### Borrows
    ///
    /// `Entities` (exclusive)
    ///
    /// ### Errors
    ///
    /// - `Entities` borrow failed.
    #[inline]
    pub fn try_build(self) -> Result<EntityId, error::GetStorage> {
        Ok(self
            .all_storages
            .try_borrow::<EntitiesViewMut<'_>>()?
            .add_entity((), ()))
    }

    /// Adds the entity to the `World`.  
    /// Unwraps errors.
    ///
    /// ### Borrows
    ///
    /// `Entities` (exclusive)
    ///
    /// ### Errors
    ///
    /// - `Entities` borrow failed.
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
            #[inline]
            pub fn with<T: 'static + Send + Sync>(self, component: T) -> EntityBuilder<'a, ($($type,)+ T,), ($($storage_type,)+ ViewMut<'a, T>,)> {
                EntityBuilder {
                    all_storages: self.all_storages,
                    all_borrow: self.all_borrow,
                    components: ($(self.components.$index,)+ component,),
                    storages: PhantomData,
                }
            }

            /// Adds a new `!Send` component to the future entity.
            #[cfg(feature = "non_send")]
            #[cfg_attr(docsrs, doc(cfg(feature = "non_send")))]
            #[inline]
            pub fn with_non_send<T: 'static + Sync>(self, component: T) -> EntityBuilder<'a, ($($type,)+ T,), ($($storage_type,)+ NonSend<ViewMut<'a, T>>,)> {
                EntityBuilder {
                    all_storages: self.all_storages,
                    all_borrow: self.all_borrow,
                    components: ($(self.components.$index,)+ component,),
                    storages: PhantomData,
                }
            }

            /// Adds a new `!Sync` component to the future entity.
            #[cfg(feature = "non_sync")]
            #[cfg_attr(docsrs, doc(cfg(feature = "non_sync")))]
            #[inline]
            pub fn with_non_sync<T: 'static + Send>(self, component: T) -> EntityBuilder<'a, ($($type,)+ T,), ($($storage_type,)+ NonSync<ViewMut<'a, T>>,)> {
                EntityBuilder {
                    all_storages: self.all_storages,
                    all_borrow: self.all_borrow,
                    components: ($(self.components.$index,)+ component,),
                    storages: PhantomData,
                }
            }

            /// Adds a new `!Send + !Sync` component to the future entity.
            #[cfg(all(feature = "non_send", feature = "non_sync"))]
            #[cfg_attr(docsrs, doc(cfg(all(feature = "non_send", feature = "non_sync"))))]
            #[inline]
            pub fn with_non_send_sync<T: 'static>(self, component: T) -> EntityBuilder<'a, ($($type,)+ T,), ($($storage_type,)+ NonSendSync<ViewMut<'a, T>>,)> {
                EntityBuilder {
                    all_storages: self.all_storages,
                    all_borrow: self.all_borrow,
                    components: ($(self.components.$index,)+ component,),
                    storages: PhantomData,
                }
            }

            /// Adds the entity to the `World`.
            ///
            /// ### Borrows
            ///
            /// - Each storage for this entity
            /// - `Entities` (exclusive)
            ///
            /// ### Errors
            ///
            /// - Storage borrow failed.
            /// - `Entities` borrow failed.
            #[inline]
            pub fn try_build(self) -> Result<EntityId, error::GetStorage> where ($($storage_type,)+): AllStoragesBorrow<'a>, $($storage_type: AsMut<ViewMut<'a, $type>>),+ {
                let mut storages = self.all_storages.try_borrow::<($($storage_type,)+)>()?;

                Ok(self
                    .all_storages
                    .try_borrow::<EntitiesViewMut<'_>>()?
                    .add_entity(($(storages.$index.as_mut(),)+), self.components))
            }

            /// Adds the entity to the `World`.
            /// Unwraps errors.
            ///
            /// ### Borrows
            ///
            /// - Each storage for this entity
            /// - `Entities` (exclusive)
            ///
            /// ### Errors
            ///
            /// - Storage borrow failed.
            /// - `Entities` borrow failed.
            #[cfg(feature = "panic")]
            #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
            #[track_caller]
            #[inline]
            pub fn build(self) -> EntityId where ($($storage_type,)+): AllStoragesBorrow<'a>, $($storage_type: AsMut<ViewMut<'a, $type>>),+ {
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
            /// Adds the entity to the `World`.
            ///
            /// ### Borrows
            ///
            /// - Each storage for this entity
            /// - `Entities` (exclusive)
            ///
            /// ### Errors
            ///
            /// - Storage borrow failed.
            /// - `Entities` borrow failed.
            pub fn try_build(self) -> Result<EntityId, error::GetStorage> where ($($storage_type,)+): AllStoragesBorrow<'a>, $($storage_type: AsMut<ViewMut<'a, $type>>),+ {
                let mut storages = self.all_storages.try_borrow::<($($storage_type,)+)>()?;

                Ok(self
                    .all_storages
                    .try_borrow::<EntitiesViewMut<'_>>()?
                    .add_entity(($(storages.$index.as_mut(),)+), self.components))
            }

            /// Adds the entity to the `World`.
            /// Unwraps errors.
            ///
            /// ### Borrows
            ///
            /// - Each storage for this entity
            /// - `Entities` (exclusive)
            ///
            /// ### Errors
            ///
            /// - Storage borrow failed.
            /// - `Entities` borrow failed.
            #[cfg(feature = "panic")]
            #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
            #[track_caller]
            #[inline]
            pub fn build(self) -> EntityId where ($($storage_type,)+): AllStoragesBorrow<'a>, $($storage_type: AsMut<ViewMut<'a, $type>>),+ {
                match self.try_build() {
                    Ok(id) => id,
                    Err(err) => panic!("{:?}", err),
                }
            }
        }
    }
}

entity_builder![(A, AS, 0); (B, BS, 1) (C, CS, 2) (D, DS, 3) (E, ES, 4) (F, FS, 5) (G, GS, 6) (H, HS, 7) (I, IS, 8) (J, JS, 9)];
