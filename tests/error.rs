//! `SystemError` conversions and the error trait implementations of shipyard's errors.
//!
//! Nothing here needs shipyard's `std` feature, run it with `--no-default-features` too.

use core::error::Error;
use core::fmt::{Display, Formatter};
use shipyard::advanced::StorageId;
use shipyard::error::{self, SystemError};
use shipyard::scheduler::Label;
use shipyard::{Component, View, ViewMut, Workload, World, track};

/// Error implementing only the core trait.
#[derive(Debug, PartialEq)]
struct Typed(u32);

impl Display for Typed {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "typed {}", self.0)
    }
}

impl Error for Typed {}

/// Fails with `Typed(value)`, propagated with `?`.
fn typed(value: u32) -> Result<(), SystemError> {
    Err(Typed(value))?;

    Ok(())
}

/// Asserts that a typed error propagated with `?` can be downcast through `as_error`.
#[test]
fn typed_as_error() {
    let error = typed(1).unwrap_err();

    assert_eq!(error.as_error().downcast_ref::<Typed>(), Some(&Typed(1)));
}

/// Asserts that a typed error propagated with `?` can be downcast through `into_inner`.
#[test]
fn typed_into_inner() {
    let error = typed(2).unwrap_err();

    assert_eq!(
        error.into_inner().downcast::<Typed>().ok(),
        Some(Box::new(Typed(2)))
    );
}

/// Asserts that a `SystemError` displays and debugs like the error it holds.
#[test]
fn typed_formatting() {
    let error = typed(3).unwrap_err();

    assert_eq!(
        (format!("{error}"), format!("{error:?}")),
        ("typed 3".to_string(), "Typed(3)".to_string())
    );
}

/// Asserts that an already boxed error becomes the payload itself, it isn't boxed again.
#[test]
fn boxed() {
    fn boxed() -> Result<(), SystemError> {
        let error: Box<dyn Error + Send + Sync> = Box::new(Typed(4));
        Err(error)?;

        Ok(())
    }

    let error = boxed().unwrap_err();

    assert_eq!(
        error.into_inner().downcast::<Typed>().ok(),
        Some(Box::new(Typed(4)))
    );
}

/// Asserts that a `SystemError` propagated with `?` keeps its payload.
#[test]
fn propagated() {
    fn propagated() -> Result<(), SystemError> {
        typed(5)?;

        Ok(())
    }

    let error = propagated().unwrap_err();

    assert_eq!(error.as_error().downcast_ref::<Typed>(), Some(&Typed(5)));
}

/// Asserts that an `anyhow::Error` converts into a `SystemError` with `?`.
#[test]
fn anyhow() {
    fn anyhow() -> Result<(), SystemError> {
        Err(anyhow::anyhow!("anyhow failure"))?;

        Ok(())
    }

    let error = anyhow().unwrap_err();

    assert_eq!(error.to_string(), "anyhow failure");
}

/// Compiles only when `T` is `Send + Sync + 'static`.
fn send_sync<T: Send + Sync + 'static>() {}

/// Compiles only when `T` implements the core `Error` trait.
fn core_error<T: Error>() {}

/// Asserts that `SystemError` can be sent and shared between threads.
#[test]
fn system_error_send_sync() {
    send_sync::<SystemError>();
}

/// Asserts that shipyard's error types implement the core `Error` trait.
#[test]
fn core_errors() {
    core_error::<error::Borrow>();
    core_error::<error::GetStorage>();
    core_error::<error::RegroupSet>();
    core_error::<error::NewEntity>();
    core_error::<error::AddComponent>();
    core_error::<error::AddWorkload>();
    core_error::<error::SetDefaultWorkload>();
    core_error::<error::RunWorkload>();
    core_error::<error::Run>();
    core_error::<error::MissingComponent>();
    core_error::<error::InvalidSystem>();
    core_error::<error::UniqueRemove>();
    core_error::<error::Apply>();
    core_error::<error::UniquePresence>();
    core_error::<error::CustomStorageView>();
    core_error::<error::ImpossibleRequirements>();
    core_error::<error::GetComponent>();
}

/// Asserts that the serialization errors implement the core `Error` trait.
#[cfg(feature = "serde1")]
#[test]
fn core_serde_errors() {
    core_error::<error::Serialize<&mut serde_json::Serializer<Vec<u8>>>>();
    core_error::<
        error::Deserialize<
            'static,
            &mut serde_json::Deserializer<serde_json::de::StrRead<'static>>,
        >,
    >();
}

/// Asserts that `Run` and `RunWorkload` can be sent and shared between threads.
#[test]
fn run_errors_send_sync() {
    send_sync::<error::Run>();
    send_sync::<error::RunWorkload>();
}

/// Returns the error of the system `"system"` returning `Typed(value)`.
fn custom(value: u32) -> error::RunWorkload {
    let label: Box<dyn Label> = Box::new("system");

    error::RunWorkload::Run((label, error::Run::from_custom(Typed(value))))
}

/// Asserts that a system's custom error is the source of its `Run`, itself the source of the workload error.
#[test]
fn custom_chain() {
    let failure = custom(1);

    let run = failure.source().unwrap().downcast_ref::<error::Run>();
    let typed = run.unwrap().source().unwrap().downcast_ref::<Typed>();

    assert_eq!(typed, Some(&Typed(1)));
}

/// Asserts that `custom_error` returns the system's error.
#[test]
fn custom_error() {
    let typed = custom(2).custom_error().unwrap().downcast::<Typed>();

    assert_eq!(typed.ok(), Some(Box::new(Typed(2))));
}

/// Asserts that a custom view's error is the source of its `GetStorage`.
#[test]
fn get_storage_custom() {
    let get_storage = error::GetStorage::from_custom(Typed(3));

    assert_eq!(
        get_storage.source().unwrap().downcast_ref::<Typed>(),
        Some(&Typed(3))
    );
}

/// Asserts that a failed regroup's `GetStorage` is the source of the workload error,
/// and its borrow error is the source of the `GetStorage`.
#[test]
fn regroup_chain() {
    let failure = error::RunWorkload::Regroup(error::GetStorage::Entities(error::Borrow::Unique));

    let get_storage = failure
        .source()
        .unwrap()
        .downcast_ref::<error::GetStorage>();
    let borrow = get_storage
        .unwrap()
        .source()
        .unwrap()
        .downcast_ref::<error::Borrow>();

    assert_eq!(borrow, Some(&error::Borrow::Unique));
}

/// Component the workloads below borrow.
struct Value;

impl Component for Value {
    type Tracking = track::Untracked;
}

/// Reads `Value`.
fn read(_: View<'_, Value>) {}

/// Returns the error of a workload reading `Value` while it's exclusively borrowed.
fn borrowed() -> error::RunWorkload {
    let world = World::builder()
        .with_custom_lock::<parking_lot::RawRwLock>()
        .build();
    Workload::new("read")
        .with_system(read)
        .add_to_world(&world)
        .unwrap();
    let _value = world.borrow::<ViewMut<'_, Value>>().unwrap();

    world.run_workload("read").unwrap_err()
}

/// Asserts that a workload failing to borrow a storage has the chain
/// `RunWorkload` -> `Run` -> `GetStorage` -> `Borrow`, and that the borrow error is the last link.
#[test]
fn borrow_chain() {
    let failure = borrowed();

    let run = failure.source().unwrap().downcast_ref::<error::Run>();
    let get_storage = run
        .unwrap()
        .source()
        .unwrap()
        .downcast_ref::<error::GetStorage>();
    let borrow = get_storage
        .unwrap()
        .source()
        .unwrap()
        .downcast_ref::<error::Borrow>();

    assert_eq!(borrow, Some(&error::Borrow::Shared));
    assert!(borrow.unwrap().source().is_none());
}

/// Asserts that a workload failing to borrow a storage has no custom error.
#[test]
fn borrow_custom_error() {
    assert!(borrowed().custom_error().is_none());
}

/// Asserts that the variants without an underlying cause have no source.
#[test]
fn leaves() {
    let missing = error::GetStorage::MissingStorage {
        name: None,
        id: StorageId::of::<Value>(),
    };
    let tracking = error::GetStorage::TrackingNotEnabled {
        name: None,
        id: StorageId::of::<Value>(),
        tracking: "Insertion",
    };

    assert!(missing.source().is_none());
    assert!(tracking.source().is_none());
    assert!(error::RunWorkload::Scheduler.source().is_none());
    assert!(error::RunWorkload::MissingWorkload.source().is_none());
}

/// Asserts that a workload error converts into an `anyhow::Error`, which walks the whole chain.
#[test]
fn anyhow_chain() {
    let failure: anyhow::Error = borrowed().into();

    assert_eq!(failure.chain().count(), 4);
    assert_eq!(
        failure.root_cause().downcast_ref::<error::Borrow>(),
        Some(&error::Borrow::Shared)
    );
}

/// Asserts that a custom error's `Display` only shows each level, `Debug` the whole message.
#[test]
fn custom_formatting() {
    let failure = custom(4);
    let run = failure.source().unwrap();

    assert_eq!(
        [
            format!("{failure}"),
            format!("{run}"),
            format!("{failure:?}")
        ],
        [
            "system failed.",
            "The system returned an error.",
            "system failed: run failed with a custom error, Typed(4).",
        ]
    );
}

/// Asserts that a borrow error's `Display` only shows each level, `Debug` the whole message.
#[test]
fn borrow_formatting() {
    let failure = error::Run::GetStorage(error::GetStorage::Entities(error::Borrow::Unique));
    let get_storage = failure.source().unwrap();

    assert_eq!(
        [
            format!("{failure}"),
            format!("{get_storage}"),
            format!("{failure:?}")
        ],
        [
            "Couldn't access a storage.",
            "Cannot borrow Entities storage.",
            "Cannot mutably borrow Entities storage while it's already borrowed.",
        ]
    );
}
