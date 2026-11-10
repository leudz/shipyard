//! Compile-fail checks, rustdoc runs them as doc tests.

/// Asserts that an error that isn't `Send` can't become a `SystemError`.
///
/// ```compile_fail,E0277
/// use core::fmt::{Display, Formatter};
/// use shipyard::error::SystemError;
/// use std::rc::Rc;
///
/// #[derive(Debug)]
/// struct NotSend(Rc<u8>);
///
/// impl Display for NotSend {
///     fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
///         f.write_str("not send")
///     }
/// }
///
/// impl core::error::Error for NotSend {}
///
/// let _ = SystemError::from(NotSend(Rc::new(0)));
/// ```
struct NotSendPayload;

/// Asserts that an error that isn't `Sync` can't become a `SystemError`.
///
/// ```compile_fail,E0277
/// use core::cell::Cell;
/// use core::fmt::{Display, Formatter};
/// use shipyard::error::SystemError;
///
/// #[derive(Debug)]
/// struct NotSync(Cell<u8>);
///
/// impl Display for NotSync {
///     fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
///         f.write_str("not sync")
///     }
/// }
///
/// impl core::error::Error for NotSync {}
///
/// let _ = SystemError::from(NotSync(Cell::new(0)));
/// ```
struct NotSyncPayload;
