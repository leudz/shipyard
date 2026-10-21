use super::pending::{world, write_a};
use super::*;
use crate::sparse_set::SparseSet;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, Once};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record as Values};
use tracing::{Event, Metadata, Subscriber};

/// A span or an event, its name followed by its fields as `name=value`.
struct Record {
    /// 0 for events.
    id: u64,
    parent: Option<u64>,
    text: String,
}

/// Every span and event of the test binary, once a test installed `Recorder`.
static RECORDS: Mutex<Vec<Record>> = Mutex::new(Vec::new());

/// Keeps every span and event in `RECORDS`.
///
/// It can only be the global subscriber, `tracing` isn't built with `std`.
/// We tell the tests apart by their workload's name.
struct Recorder {
    next_id: AtomicU64,
}

impl Subscriber for Recorder {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, span: &Attributes<'_>) -> Id {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut text = String::from(span.metadata().name());
        span.record(&mut Fields(&mut text));

        RECORDS.lock().unwrap().push(Record {
            id,
            parent: span.parent().map(Id::into_u64),
            text,
        });

        Id::from_u64(id)
    }

    fn record(&self, span: &Id, values: &Values<'_>) {
        let mut records = RECORDS.lock().unwrap();
        let record = records
            .iter_mut()
            .find(|record| record.id == span.into_u64())
            .unwrap();

        values.record(&mut Fields(&mut record.text));
    }

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut text = String::from("event");
        event.record(&mut Fields(&mut text));

        RECORDS.lock().unwrap().push(Record {
            id: 0,
            parent: event.parent().map(Id::into_u64),
            text,
        });
    }

    fn enter(&self, _: &Id) {}

    fn exit(&self, _: &Id) {}
}

/// Writes fields at the end of a record's text.
struct Fields<'a>(&'a mut String);

impl Visit for Fields<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn core::fmt::Debug) {
        write!(self.0, " {}={:?}", field.name(), value).unwrap();
    }
}

/// Runs the registered workload `name` and returns what it reported, in order.
fn run_traced(world: &World, name: &'static str) -> Vec<String> {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        tracing::subscriber::set_global_default(Recorder {
            next_id: AtomicU64::new(1),
        })
        .unwrap();
    });

    world.run_workload(name).unwrap();

    let records = RECORDS.lock().unwrap();
    let workload = records
        .iter()
        .find(|record| record.text.starts_with("workload") && record.text.contains(name))
        .unwrap();

    records
        .iter()
        .filter(|record| record.parent == Some(workload.id))
        .map(|record| record.text.clone())
        .collect()
}

/// Asserts that a workload's regroups are traced with their whole set and trigger,
/// and whether the set was already clean.
#[test]
fn regroups() {
    let mut world = world();
    world.add_entity((A, B));
    Workload::new("traced_regroups")
        .with_system(write_a)
        .add_to_world(&world)
        .unwrap();

    let records = run_traced(&world, "traced_regroups");

    assert_eq!(records.len(), 3);
    // The entry regroup has an entity to move, `write_a` only changes values.
    assert!(records[0].starts_with("regroup"));
    assert!(records[0].contains(type_name::<SparseSet<B>>()));
    assert!(records[0].contains("trigger=Entry"));
    assert!(records[0].contains("clean=false"));
    assert!(records[1].starts_with("system"));
    assert!(records[2].starts_with("regroup"));
    assert!(records[2].contains("trigger=Pending"));
    assert!(records[2].contains("clean=true"));
}

/// Asserts that a system skipped by its condition and the regroup it would have needed
/// are traced as skipped.
#[test]
fn skipped() {
    let world = world();
    Workload::new("traced_skipped")
        .with_system(write_a.run_if(|| false))
        .add_to_world(&world)
        .unwrap();

    let records = run_traced(&world, "traced_skipped");

    assert_eq!(records.len(), 3);
    assert!(records[0].contains("trigger=Entry"));
    assert!(records[1].starts_with("event message=skipped system"));
    assert!(records[2].starts_with("event message=skipped regroup"));
    assert!(records[2].contains("trigger=Pending"));
}
