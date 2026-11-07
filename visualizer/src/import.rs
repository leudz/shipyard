//! Turns the bytes of an exported file into [`WorkloadsInfo`], without touching application state.

use serde_json::error::Category;
use shipyard::scheduler::info::WorkloadsInfo;

/// Why a candidate file can't be imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// The platform couldn't read the file.
    Read(String),
    /// The file isn't valid JSON.
    InvalidJson(String),
    /// The file is JSON, but not the current `WorkloadsInfo` format.
    UnsupportedFormat(String),
}

impl ImportError {
    /// A sentence describing the failure, suitable as the headline of an error message.
    pub fn summary(&self) -> &'static str {
        match self {
            ImportError::Read(_) => "The file could not be read.",
            ImportError::InvalidJson(_) => "The file is not valid JSON.",
            ImportError::UnsupportedFormat(_) => {
                "The file is not a workload export from this version of shipyard."
            }
        }
    }

    /// The underlying reason, including the position in the file when there is one.
    pub fn details(&self) -> &str {
        match self {
            ImportError::Read(details)
            | ImportError::InvalidJson(details)
            | ImportError::UnsupportedFormat(details) => details,
        }
    }
}

/// Parses the JSON produced by serializing `World::workloads_info`.
///
/// An export without workloads is valid and returns an empty [`WorkloadsInfo`].
pub fn parse_workloads(bytes: &[u8]) -> Result<WorkloadsInfo, ImportError> {
    serde_json::from_slice(bytes).map_err(|error| {
        let details = error.to_string();

        match error.classify() {
            // We parse from memory, so serde_json never does any I/O itself.
            Category::Io => ImportError::Read(details),
            Category::Syntax | Category::Eof => ImportError::InvalidJson(details),
            Category::Data => ImportError::UnsupportedFormat(details),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE_EATER: &[u8] = include_bytes!("square_eater_workloads.json");
    const OLD_FORMAT: &[u8] = include_bytes!("../../output/visualizer-design/drop_me.json");

    /// Asserts that the bundled square-eater example, which contains borrow conflicts, imports.
    #[test]
    fn square_eater() {
        let workloads = parse_workloads(SQUARE_EATER).unwrap();

        let floor_loop = &workloads.0["Workload(square_eater::floor_loop)"];
        assert_eq!(workloads.0.len(), 1);
        assert!(!floor_loop.batches_info.is_empty());
    }

    /// Asserts that an export without any workload is a valid, empty dataset.
    #[test]
    fn no_workloads() {
        let workloads = parse_workloads(b"{}").unwrap();

        assert!(workloads.0.is_empty());
    }

    /// Asserts that a workload without batches imports.
    #[test]
    fn no_batches() {
        let json = br#"{"W": {"name": "W", "batches_info": []}}"#;

        let workloads = parse_workloads(json).unwrap();

        assert!(workloads.0["W"].batches_info.is_empty());
    }

    /// Asserts that a batch without systems or regroups imports, and that `regroups` may be omitted.
    #[test]
    fn empty_batch() {
        let json = br#"{"W": {"name": "W", "batches_info": [{"systems": [null, []]}]}}"#;

        let workloads = parse_workloads(json).unwrap();

        let batch = &workloads.0["W"].batches_info[0];
        assert_eq!(batch.systems().count(), 0);
        assert!(batch.regroups.is_empty());
    }

    /// Asserts that a batch whose only work is a regroup imports, even though we don't display regroups.
    #[test]
    fn regroup() {
        let json = br#"{"W": {"name": "W", "batches_info": [{
            "systems": [null, []],
            "regroups": [{
                "borrow": [{"name": "A", "mutability": "Exclusive", "thread_safe": true}],
                "trigger": "Pending"
            }]
        }]}}"#;

        let workloads = parse_workloads(json).unwrap();

        assert_eq!(workloads.0["W"].batches_info[0].regroups.len(), 1);
    }

    /// Asserts that truncated JSON is reported as invalid JSON.
    #[test]
    fn malformed() {
        let error = parse_workloads(br#"{"W": {"name": "#).unwrap_err();

        assert!(matches!(error, ImportError::InvalidJson(_)));
    }

    /// Asserts that an empty file is reported as invalid JSON.
    #[test]
    fn empty_file() {
        let error = parse_workloads(b"").unwrap_err();

        assert!(matches!(error, ImportError::InvalidJson(_)));
    }

    /// Asserts that valid JSON with an unrelated shape is reported as an unsupported format.
    #[test]
    fn unrelated() {
        let error = parse_workloads(br#"{"version": 1, "items": []}"#).unwrap_err();

        assert!(matches!(error, ImportError::UnsupportedFormat(_)));
    }

    /// Asserts that the older `batch_info`/`type_id` export format is rejected as unsupported.
    #[test]
    fn old_format() {
        let error = parse_workloads(OLD_FORMAT).unwrap_err();

        assert!(matches!(error, ImportError::UnsupportedFormat(_)));
        assert!(error.details().contains("missing field `batches_info`"));
    }
}
