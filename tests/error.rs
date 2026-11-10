//! `SystemError` conversions and the error trait implementations of shipyard's errors.
//!
//! Nothing here needs shipyard's `std` feature, run it with `--no-default-features` too.

use core::error::Error;
use core::fmt::{Display, Formatter};
use shipyard::error::{self, SystemError};

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
