use crate::error;
use crate::storage::AllStorages;
use crate::world::World;
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::ops::Deref;

pub trait TryUniqueAdder {
    fn try_add_unique<T: 'static + Send + Sync>(&self, unique: T) -> Result<(), error::Borrow>;
    #[cfg(feature = "non_send")]
    fn try_add_unique_non_send<T: 'static + Sync>(&self, unique: T) -> Result<(), error::Borrow>;
    #[cfg(feature = "non_sync")]
    fn try_add_unique_non_sync<T: 'static + Send>(&self, unique: T) -> Result<(), error::Borrow>;
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    fn try_add_unique_non_send_sync<T: 'static>(&self, unique: T) -> Result<(), error::Borrow>;
}

pub trait UniqueAdder {
    fn add_unique<T: 'static + Send + Sync>(&self, unique: T);
    #[cfg(feature = "non_send")]
    fn add_unique_non_send<T: 'static + Sync>(&self, unique: T);
    #[cfg(feature = "non_sync")]
    fn add_unique_non_sync<T: 'static + Send>(&self, unique: T);
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    fn add_unique_non_send_sync<T: 'static>(&self, unique: T);
}

pub struct Wrap<T>(UnsafeCell<MaybeUninit<T>>);

impl<T> Wrap<T> {
    pub fn new(inner: T) -> Self {
        Wrap(UnsafeCell::new(MaybeUninit::new(inner)))
    }
}

impl TryUniqueAdder for World {
    fn try_add_unique<T: 'static + Send + Sync>(&self, unique: T) -> Result<(), error::Borrow> {
        World::try_add_unique(self, unique)
    }

    #[cfg(feature = "non_send")]
    fn try_add_unique_non_send<T: 'static + Sync>(&self, unique: T) -> Result<(), error::Borrow> {
        World::try_add_unique_non_send(self, unique)
    }

    #[cfg(feature = "non_sync")]
    fn try_add_unique_non_sync<T: 'static + Send>(&self, unique: T) -> Result<(), error::Borrow> {
        World::try_add_unique_non_sync(self, unique)
    }

    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    fn try_add_unique_non_send_sync<T: 'static>(&self, unique: T) -> Result<(), error::Borrow> {
        World::try_add_unique_non_send_sync(self, unique)
    }
}

#[cfg(feature = "panic")]
impl UniqueAdder for World {
    #[track_caller]
    fn add_unique<T: 'static + Send + Sync>(&self, unique: T) {
        match self.try_add_unique(unique) {
            Ok(_) => (),
            Err(err) => panic!("{:?}", err),
        }
    }

    #[cfg(feature = "non_send")]
    #[track_caller]
    fn add_unique_non_send<T: 'static + Sync>(&self, unique: T) {
        match self.try_add_unique_non_send(unique) {
            Ok(_) => (),
            Err(err) => panic!("{:?}", err),
        }
    }

    #[cfg(feature = "non_sync")]
    #[track_caller]
    fn add_unique_non_sync<T: 'static + Send>(&self, unique: T) {
        match self.try_add_unique_non_sync(unique) {
            Ok(_) => (),
            Err(err) => panic!("{:?}", err),
        }
    }

    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    #[track_caller]
    fn add_unique_non_send_sync<T: 'static>(&self, unique: T) {
        match self.try_add_unique_non_send_sync(unique) {
            Ok(_) => (),
            Err(err) => panic!("{:?}", err),
        }
    }
}

impl UniqueAdder for AllStorages {
    fn add_unique<T: 'static + Send + Sync>(&self, unique: T) {
        AllStorages::add_unique(self, unique);
    }

    #[cfg(feature = "non_send")]
    fn add_unique_non_send<T: 'static + Sync>(&self, unique: T) {
        AllStorages::add_unique_non_send(self, unique)
    }

    #[cfg(feature = "non_sync")]
    fn add_unique_non_sync<T: 'static + Send>(&self, unique: T) {
        AllStorages::add_unique_non_sync(self, unique);
    }

    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    fn add_unique_non_send_sync<T: 'static>(&self, unique: T) {
        AllStorages::add_unique_non_send_sync(self, unique);
    }
}

pub trait AddUnique {
    fn try_add_unique<UA: Deref<Target = UAD>, UAD: TryUniqueAdder>(
        &self,
        unique_adder: &UA,
    ) -> Result<(), error::Borrow>;
    fn add_unique<UA: Deref<Target = UAD>, UAD: UniqueAdder>(&self, unique_adder: &UA);
}

impl<T: 'static + Send + Sync> AddUnique for &&&Wrap<T> {
    fn try_add_unique<UA: Deref<Target = UAD>, UAD: TryUniqueAdder>(
        &self,
        unique_adder: &UA,
    ) -> Result<(), error::Borrow> {
        let component: T = unsafe {
            let component: *const MaybeUninit<T> = self.0.get();
            (*component).as_ptr().read()
        };

        unique_adder.try_add_unique(component)
    }

    fn add_unique<UA: Deref<Target = UAD>, UAD: UniqueAdder>(&self, unique_adder: &UA) {
        let component: T = unsafe {
            let component: *const MaybeUninit<T> = self.0.get();
            (*component).as_ptr().read()
        };

        unique_adder.add_unique(component);
    }
}

#[cfg(feature = "non_send")]
impl<T: 'static + Sync> AddUnique for &&Wrap<T> {
    fn try_add_unique<UA: Deref<Target = UAD>, UAD: TryUniqueAdder>(
        &self,
        unique_adder: &UA,
    ) -> Result<(), error::Borrow> {
        let component: T = unsafe {
            let component: *const MaybeUninit<T> = self.0.get();
            (*component).as_ptr().read()
        };

        unique_adder.try_add_unique_non_send(component)
    }

    fn add_unique<UA: Deref<Target = UAD>, UAD: UniqueAdder>(&self, unique_adder: &UA) {
        let component: T = unsafe {
            let component: *const MaybeUninit<T> = self.0.get();
            (*component).as_ptr().read()
        };

        unique_adder.add_unique_non_send(component);
    }
}

#[cfg(feature = "non_sync")]
impl<T: 'static + Send> AddUnique for &Wrap<T> {
    fn try_add_unique<UA: Deref<Target = UAD>, UAD: TryUniqueAdder>(
        &self,
        unique_adder: &UA,
    ) -> Result<(), error::Borrow> {
        let component: T = unsafe {
            let component: *const MaybeUninit<T> = self.0.get();
            (*component).as_ptr().read()
        };

        unique_adder.try_add_unique_non_sync(component)
    }

    fn add_unique<UA: Deref<Target = UAD>, UAD: UniqueAdder>(&self, unique_adder: &UA) {
        let component: T = unsafe {
            let component: *const MaybeUninit<T> = self.0.get();
            (*component).as_ptr().read()
        };

        unique_adder.add_unique_non_sync(component);
    }
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
impl<T: 'static> AddUnique for Wrap<T> {
    fn try_add_unique<UA: Deref<Target = UAD>, UAD: TryUniqueAdder>(
        &self,
        unique_adder: &UA,
    ) -> Result<(), error::Borrow> {
        let component: T = unsafe {
            let component: *const MaybeUninit<T> = self.0.get();
            (*component).as_ptr().read()
        };

        unique_adder.try_add_unique_non_send_sync(component)
    }

    fn add_unique<UA: Deref<Target = UAD>, UAD: UniqueAdder>(&self, unique_adder: &UA) {
        let component: T = unsafe {
            let component: *const MaybeUninit<T> = self.0.get();
            (*component).as_ptr().read()
        };

        unique_adder.add_unique_non_send_sync(component);
    }
}

/// Adds a new unique storage, unique storages store exactly one `T` at any time.
///
/// The advantage of the macro is it works with any `Send` and `Sync` bound.  
/// Can be used with [World] or [AllStorages].  
/// To access a unique storage value, use [UniqueView] or [UniqueViewMut].  
/// Does nothing if the storage already exists.  
/// Unwraps error when used with [World].
///
/// [World]: struct.World.html
/// [AllStorages]: struct.AllStorages.html
/// [UniqueView]: struct.UniqueView.html
/// [UniqueViewMut]: struct.UniqueViewMut.html
#[macro_export]
macro_rules! add_unique {
    ($world: expr, $component: expr) => {{
        use $crate::{AddUnique, Wrap};

        let component = Wrap::new($component);

        (&&&&component).add_unique(&$world);
    }};
}

/// Adds a new unique storage, unique storages store exactly one `T` at any time.
///
/// The advantage of the macro is it works with any `Send` and `Sync` bound.  
/// Can be used with [World] or [AllStorages].  
/// [add_unique] should be preferred with [AllStorages] since it can't fail.  
/// To access a unique storage value, use [UniqueView] or [UniqueViewMut].  
/// Does nothing if the storage already exists.
///
/// [World]: struct.World.html
/// [AllStorages]: struct.AllStorages.html
/// [UniqueView]: struct.UniqueView.html
/// [UniqueViewMut]: struct.UniqueViewMut.html
/// [add_unique]: macro.add_unique.html
#[macro_export]
macro_rules! try_add_unique {
    ($world: expr, $component: expr) => {{
        use $crate::{AddUnique, Wrap};

        let component = Wrap::new($component);

        (&&&&component).try_add_unique(&$world)
    }};
}
