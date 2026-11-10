//! Compile-fail checks, rustdoc runs them as doc tests.

/// Asserts that a `Workload` can't be turned into a sequential workload,
/// even with `IntoSequentialWorkload` in scope.
///
/// ```compile_fail,E0599
/// use shipyard::{IntoSequentialWorkload, IntoWorkload, Workload};
///
/// fn sys1() {}
/// fn sys2() {}
///
/// let workload: Workload = (sys1, sys2).into_workload();
/// workload.into_sequential_workload();
/// ```
struct WorkloadIsNotSequential;
