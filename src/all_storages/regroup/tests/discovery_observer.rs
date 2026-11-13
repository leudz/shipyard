use core::cell::Cell;

std::thread_local! {
    static CHECKS: Cell<Option<usize>> = const { Cell::new(None) };
}

/// Counts sparse discovery probes on this test thread until dropped.
pub(in super::super) struct SparseChecks;

impl SparseChecks {
    pub(in super::super) fn start() -> SparseChecks {
        CHECKS.with(|checks| assert!(checks.replace(Some(0)).is_none()));

        SparseChecks
    }

    pub(in super::super) fn count(&self) -> usize {
        CHECKS.with(Cell::get).expect("the probe counter is active")
    }
}

impl Drop for SparseChecks {
    fn drop(&mut self) {
        CHECKS.with(|checks| checks.set(None));
    }
}

/// Records a sparse probe only while a test is counting.
pub(super) fn sparse_check() {
    CHECKS.with(|checks| {
        if let Some(count) = checks.get() {
            checks.set(Some(count + 1));
        }
    });
}
