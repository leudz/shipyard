/// Indicates that a `struct` or `enum` can be store in the `World`.
#[cfg(feature = "thread_local")]
pub trait Component: Sized + 'static {}
/// Indicates that a `struct` or `enum` can be store in the `World`.
#[cfg(not(feature = "thread_local"))]
pub trait Component: Sized + Send + Sync + 'static {}

/// Indicates that a `struct` or `enum` can be store a single time in the `World`.
#[cfg(feature = "thread_local")]
pub trait Unique: Sized + 'static {}
/// Indicates that a `struct` or `enum` can be store a single time in the `World`.
#[cfg(not(feature = "thread_local"))]
pub trait Unique: Sized + Send + Sync + 'static {}
