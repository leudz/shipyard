//! Helpers for tests writing exports by hand.

use serde_json::{json, Value};

/// Gives each system and regroup of a workload's `batches` without an occurrence one,
/// numbered by position: it runs with what it borrows and nothing has to run before it.
///
/// A batch without `regroups` gets an empty list, the export always has one.
pub fn add_occurrences(batches: &mut [Value]) {
    let mut systems = 0;
    let mut regroups = 0;
    let mut add_system = |system: &mut Value, thread_bound: bool| {
        if system.get("occurrence").is_none() {
            system["occurrence"] = json!({
                "id": systems,
                "conditions": [],
                "footprint": system["borrow"].clone(),
                "thread_bound": thread_bound,
                "after": [],
            });
        }
        systems += 1;
    };

    for batch in batches {
        if batch.get("regroups").is_none() {
            batch["regroups"] = json!([]);
        }
        if batch["systems"][0].is_object() {
            add_system(&mut batch["systems"][0], true);
        }
        for system in batch["systems"][1].as_array_mut().unwrap() {
            add_system(system, false);
        }
        for regroup in batch
            .get_mut("regroups")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if regroup.get("occurrence").is_none() {
                let thread_bound = regroup["borrow"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|borrow| borrow["thread_safe"] == false);
                regroup["occurrence"] = json!({
                    "id": regroups,
                    "thread_bound": thread_bound,
                    "after": [],
                });
            }
            regroups += 1;
        }
    }
}
