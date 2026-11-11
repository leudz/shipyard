use super::super::*;
use super::pending::{read_a, write_a};
use super::{A, B, C, Schedule, steps};
use crate::borrow::Mutability;
use crate::scheduler::info::{
    BatchInfo, BeforeAfterConstraint, Conflict, OccurrenceRef, OrderingOrigin, Predecessor,
    RegroupInfo, RegroupOccurrence, RegroupTrigger, SystemInfo, SystemOccurrence,
};
use crate::scheduler::system_modificator::SystemModificator;
use crate::scheduler::{IntoSequentialWorkload, WorkloadModificator};
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
use crate::views::{View, ViewMut};
use crate::world::World;
use alloc::format;
use alloc::string::{String, ToString};

#[allow(unused)]
struct Usize(usize);
impl Component for Usize {
    type Tracking = crate::track::Untracked;
}
#[allow(unused)]
struct U32(u32);
impl Component for U32 {
    type Tracking = crate::track::Untracked;
}

/// Stands in for the occurrence information, in tests checking the other fields.
fn unchecked() -> SystemOccurrence {
    SystemOccurrence {
        id: usize::MAX,
        conditions: vec![],
        footprint: vec![],
        thread_bound: false,
        after: vec![],
    }
}

/// `system` with its occurrence information replaced by [`unchecked`], to check the other fields alone.
fn legacy(system: &SystemInfo) -> SystemInfo {
    let mut system = system.clone();
    system.occurrence = unchecked();

    system
}

#[test]
fn before_all() {
    fn sys_a() {}
    fn sys_b() {}

    let (_, info) = Workload::new("workload 1")
        .with_system(sys_a)
        .with_system(sys_b.before_all(sys_a))
        .build()
        .unwrap();

    assert_eq!(info.name, "workload 1");
    assert_eq!(info.batches_info.len(), 2);

    let mut systems0 = info.batches_info[0].systems();
    assert_eq!(
        systems0.next().map(legacy),
        Some(SystemInfo {
            name: "System(shipyard::scheduler::workload::tests::info_tests::before_all::sys_b)"
                .to_string(),
            borrow: vec![],
            conflict: None,
            after: vec![],
            after_all: vec![],
            before_all: vec![BeforeAfterConstraint {
                other_system: 0,
                constraint:
                    "System(shipyard::scheduler::workload::tests::info_tests::before_all::sys_a)"
                        .to_string()
            }],
            unique_id: 1,
            occurrence: unchecked()
        })
    );
    assert_eq!(systems0.next(), None);

    let mut systems1 = info.batches_info[1].systems();
    assert_eq!(
        systems1.next().map(legacy),
        Some(SystemInfo {
            name: "System(shipyard::scheduler::workload::tests::info_tests::before_all::sys_a)"
                .to_string(),
            borrow: vec![],
            conflict: None,
            after: vec![1],
            after_all: vec![],
            before_all: vec![],
            unique_id: 0,
            occurrence: unchecked()
        })
    );
    assert_eq!(systems1.next(), None);
}

#[test]
fn after_all() {
    fn sys_a() {}
    fn sys_b() {}

    let (_, info) = Workload::new("workload 1")
        .with_system(sys_a.after_all(sys_b))
        .with_system(sys_b)
        .build()
        .unwrap();

    assert_eq!(info.name, "workload 1");
    assert_eq!(info.batches_info.len(), 2);

    let mut systems0 = info.batches_info[0].systems();
    assert_eq!(
        systems0.next().map(legacy),
        Some(SystemInfo {
            name: "System(shipyard::scheduler::workload::tests::info_tests::after_all::sys_b)"
                .to_string(),
            borrow: vec![],
            conflict: None,
            after: vec![],
            after_all: vec![],
            before_all: vec![],
            unique_id: 1,
            occurrence: unchecked()
        })
    );
    assert_eq!(systems0.next(), None);

    let mut systems1 = info.batches_info[1].systems();
    assert_eq!(
        systems1.next().map(legacy),
        Some(SystemInfo {
            name: "System(shipyard::scheduler::workload::tests::info_tests::after_all::sys_a)"
                .to_string(),
            borrow: vec![],
            conflict: None,
            after: vec![1],
            after_all: vec![BeforeAfterConstraint {
                other_system: 1,
                constraint:
                    "System(shipyard::scheduler::workload::tests::info_tests::after_all::sys_b)"
                        .to_string()
            }],
            before_all: vec![],
            unique_id: 0,
            occurrence: unchecked()
        })
    );
    assert_eq!(systems1.next(), None);
}

#[test]
fn borrow() {
    fn sys_a(_: View<'_, Usize>, _: View<'_, U32>) {}
    fn sys_b(_: ViewMut<'_, Usize>, _: View<'_, U32>) {}
    fn sys_c(_: View<'_, Usize>) {}
    fn sys_d(_: View<'_, Usize>) {}

    let (_, info) = Workload::new("")
        .with_system(sys_a)
        .with_system(sys_b)
        .with_system(sys_c)
        .with_system(sys_d)
        .build()
        .unwrap();

    let mut systems0 = info.batches_info[0].systems();
    assert_eq!(
        systems0.next().map(legacy),
        Some(SystemInfo {
            name: format!("{:?}", sys_a.as_label()),
            borrow: vec![
                TypeInfo {
                    name: type_name::<SparseSet::<Usize>>().into(),
                    mutability: Mutability::Shared,
                    storage_id: StorageId::of::<SparseSet<Usize>>(),
                    thread_safe: true
                },
                TypeInfo {
                    name: type_name::<SparseSet::<U32>>().into(),
                    mutability: Mutability::Shared,
                    storage_id: StorageId::of::<SparseSet<U32>>(),
                    thread_safe: true
                }
            ],
            conflict: None,
            after: vec![],
            after_all: vec![],
            before_all: vec![],
            unique_id: 0,
            occurrence: unchecked()
        })
    );

    let mut systems1 = info.batches_info[1].systems();
    assert_eq!(
        systems1.next().map(legacy),
        Some(SystemInfo {
            name: format!("{:?}", sys_b.as_label()),
            borrow: vec![
                TypeInfo {
                    name: type_name::<SparseSet::<Usize>>().into(),
                    mutability: Mutability::Exclusive,
                    storage_id: StorageId::of::<SparseSet<Usize>>(),
                    thread_safe: true
                },
                TypeInfo {
                    name: type_name::<SparseSet::<U32>>().into(),
                    mutability: Mutability::Shared,
                    storage_id: StorageId::of::<SparseSet<U32>>(),
                    thread_safe: true
                }
            ],
            conflict: Some(Conflict::Borrow {
                type_info: TypeInfo {
                    name: type_name::<SparseSet::<Usize>>().into(),
                    mutability: Mutability::Exclusive,
                    storage_id: StorageId::of::<SparseSet<Usize>>(),
                    thread_safe: true
                },
                other_system: 0,
                other_type_info: TypeInfo {
                    name: type_name::<SparseSet::<Usize>>().into(),
                    mutability: Mutability::Shared,
                    storage_id: StorageId::of::<SparseSet<Usize>>(),
                    thread_safe: true
                }
            }),
            after: vec![0],
            after_all: vec![],
            before_all: vec![],
            unique_id: 1,
            occurrence: unchecked()
        })
    );

    let mut systems2 = info.batches_info[2].systems();
    assert_eq!(
        systems2.next().map(legacy),
        Some(SystemInfo {
            name: format!("{:?}", sys_c.as_label()),
            borrow: vec![TypeInfo {
                name: type_name::<SparseSet::<Usize>>().into(),
                mutability: Mutability::Shared,
                storage_id: StorageId::of::<SparseSet<Usize>>(),
                thread_safe: true
            },],
            conflict: Some(Conflict::Borrow {
                type_info: TypeInfo {
                    name: type_name::<SparseSet::<Usize>>().into(),
                    mutability: Mutability::Shared,
                    storage_id: StorageId::of::<SparseSet<Usize>>(),
                    thread_safe: true
                },
                other_system: 1,
                other_type_info: TypeInfo {
                    name: type_name::<SparseSet::<Usize>>().into(),
                    mutability: Mutability::Exclusive,
                    storage_id: StorageId::of::<SparseSet<Usize>>(),
                    thread_safe: true
                }
            }),
            after: vec![1],
            after_all: vec![],
            before_all: vec![],
            unique_id: 2,
            occurrence: unchecked()
        })
    );
    assert_eq!(
        systems2.next().map(legacy),
        Some(SystemInfo {
            name: format!("{:?}", sys_d.as_label()),
            borrow: vec![TypeInfo {
                name: type_name::<SparseSet::<Usize>>().into(),
                mutability: Mutability::Shared,
                storage_id: StorageId::of::<SparseSet<Usize>>(),
                thread_safe: true
            },],
            conflict: Some(Conflict::Borrow {
                type_info: TypeInfo {
                    name: type_name::<SparseSet::<Usize>>().into(),
                    mutability: Mutability::Shared,
                    storage_id: StorageId::of::<SparseSet<Usize>>(),
                    thread_safe: true
                },
                other_system: 1,
                other_type_info: TypeInfo {
                    name: type_name::<SparseSet::<Usize>>().into(),
                    mutability: Mutability::Exclusive,
                    storage_id: StorageId::of::<SparseSet<Usize>>(),
                    thread_safe: true
                }
            }),
            after: vec![1],
            after_all: vec![],
            before_all: vec![],
            unique_id: 3,
            occurrence: unchecked()
        })
    );
}

/// Returns the information of `T`'s storage, borrowed with `mutability`.
fn type_info<T: Component + Send + Sync>(mutability: Mutability) -> TypeInfo {
    TypeInfo {
        name: type_name::<SparseSet<T>>().into(),
        mutability,
        storage_id: StorageId::of::<SparseSet<T>>(),
        thread_safe: true,
    }
}

/// Returns a world with the AB and BC groups, forming the ABC connected set.
fn abc_world() -> World {
    World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(B, C)>()
        .build()
}

/// Adds `workload` to `world` and returns its batches information.
fn registered_batches(workload: Workload, world: &World) -> Vec<BatchInfo> {
    workload.add_to_world(world).unwrap();

    world.workloads_info().0[""].batches_info.clone()
}

/// Asserts that a registered workload's regroup borrows its whole connected set exclusively,
/// including the storages its systems don't borrow.
#[test]
fn regroup_borrow() {
    let world = abc_world();

    let batches_info = registered_batches(Workload::new("").with_system(read_a), &world);

    let mut set = vec![
        type_info::<A>(Mutability::Exclusive),
        type_info::<B>(Mutability::Exclusive),
        type_info::<C>(Mutability::Exclusive),
    ];
    set.sort();
    assert_eq!(
        batches_info[0].regroups,
        &[RegroupInfo {
            borrow: set,
            trigger: RegroupTrigger::Entry,
            occurrence: RegroupOccurrence {
                id: 0,
                thread_bound: false,
                after: vec![],
            },
        }]
    );
    assert_eq!(batches_info[0].systems().count(), 0);
}

/// Asserts that a registered workload writing a grouped storage shows its entry regroup,
/// the system, and the regroup ending the workload, each in its own batch.
#[test]
fn entry_exit() {
    let world = abc_world();

    let batches_info = registered_batches(Workload::new("").with_system(write_a), &world);

    let triggers: Vec<Vec<RegroupTrigger>> = batches_info
        .iter()
        .map(|batch_info| {
            batch_info
                .regroups
                .iter()
                .map(|regroup| regroup.trigger)
                .collect()
        })
        .collect();
    assert_eq!(
        triggers,
        [
            vec![RegroupTrigger::Entry],
            vec![],
            vec![RegroupTrigger::Pending]
        ]
    );
    assert_eq!(batches_info[1].systems().count(), 1);
}

/// Asserts that a split batch shows the regroup next to the system that doesn't need it,
/// and the system waiting for it in the next batch, with a conflict naming the regroup.
#[test]
fn split() {
    fn write_u32(_: ViewMut<'_, U32>) {}
    let world = abc_world();
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(read_a)
        .with_system(write_u32.after_all(write_a));

    let batches_info = registered_batches(workload, &world);

    assert_eq!(batches_info.len(), 4);
    let split: Vec<&str> = batches_info[2]
        .systems()
        .map(|system| &*system.name)
        .collect();
    assert_eq!(split, [format!("{:?}", write_u32.as_label())]);
    assert_eq!(batches_info[2].regroups.len(), 1);
    let waiting = batches_info[3].systems().next().unwrap();
    assert_eq!(waiting.name, format!("{:?}", read_a.as_label()));
    assert_eq!(
        waiting.conflict,
        Some(Conflict::Regroup(type_info::<A>(Mutability::Shared)))
    );
}

/// Asserts that a conflict naming a system of the previous batch stays,
/// even when that batch also has a regroup the system waits for.
#[test]
fn kept_conflict() {
    fn write_u32(_: ViewMut<'_, U32>) {}
    fn read_a_u32(_: View<'_, A>, _: View<'_, U32>) {}
    let world = abc_world();
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(write_u32.after_all(write_a))
        .with_system(read_a_u32);

    let batches_info = registered_batches(workload, &world);

    assert_eq!(batches_info.len(), 4);
    assert_eq!(batches_info[2].regroups.len(), 1);
    let reader = batches_info[3].systems().next().unwrap();
    assert!(matches!(
        &reader.conflict,
        Some(Conflict::Borrow {
            other_system: 1,
            ..
        })
    ));
}

/// Asserts that two occurrences of the same system stay two entries,
/// the second one waiting for the regroup of the first one's write.
#[test]
fn repeated() {
    let world = abc_world();
    let workload = Workload::new("").with_system(write_a).with_system(write_a);

    let batches_info = registered_batches(workload, &world);

    let systems: Vec<&SystemInfo> = batches_info.iter().flat_map(BatchInfo::systems).collect();
    assert_eq!(systems.len(), 2);
    assert_eq!(systems[0].name, systems[1].name);
    assert_eq!(batches_info[2].regroups.len(), 1);
    assert_eq!(
        systems[1].conflict,
        Some(Conflict::Regroup(type_info::<A>(Mutability::Exclusive)))
    );
}

/// Asserts that a registered workload's information with regroups goes through serde unchanged.
#[cfg(feature = "serde1")]
#[test]
fn serde_regroups() {
    let world = abc_world();
    Workload::new("")
        .with_system(write_a)
        .add_to_world(&world)
        .unwrap();
    let info = world.workloads_info().0[""].clone();

    let json = serde_json::to_string(&info).unwrap();
    let deserialized: WorkloadInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(serde_json::to_string(&deserialized).unwrap(), json);
    assert_eq!(deserialized.batches_info[0].regroups.len(), 1);
}

/// Asserts that a system reference and a regroup reference with the same number stay different through serde.
#[cfg(feature = "serde1")]
#[test]
fn serde_references() {
    let references = vec![OccurrenceRef::System(3), OccurrenceRef::Regroup(3)];

    let json = serde_json::to_string(&references).unwrap();
    let deserialized: Vec<OccurrenceRef> = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized, references);
    assert_ne!(deserialized[0], deserialized[1]);
}

/// Asserts that a predecessor keeps all of its origins, labels included, through serde.
#[cfg(feature = "serde1")]
#[test]
fn serde_several_origins() {
    let predecessor = Predecessor {
        occurrence: OccurrenceRef::System(1),
        origins: vec![
            OrderingOrigin::Implicit,
            OrderingOrigin::BeforeAll("label".to_string()),
            OrderingOrigin::Sequential,
        ],
    };

    let json = serde_json::to_string(&predecessor).unwrap();
    let deserialized: Predecessor = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized, predecessor);
}

/// The occurrence information of the system whose occurrence id is `id`.
fn system_occurrence(batches_info: &[BatchInfo], id: usize) -> (&SystemInfo, &SystemOccurrence) {
    batches_info
        .iter()
        .flat_map(BatchInfo::systems)
        .find_map(|system| {
            let occurrence = &system.occurrence;

            (occurrence.id == id).then_some((system, occurrence))
        })
        .unwrap()
}

/// The predecessors' references of the system whose occurrence id is `id`.
fn after_refs(batches_info: &[BatchInfo], id: usize) -> Vec<OccurrenceRef> {
    let (_, occurrence) = system_occurrence(batches_info, id);

    occurrence
        .after
        .iter()
        .map(|predecessor| predecessor.occurrence)
        .collect()
}

/// Every regroup in batch order.
fn regroups(batches_info: &[BatchInfo]) -> Vec<&RegroupInfo> {
    batches_info
        .iter()
        .flat_map(|batch| &batch.regroups)
        .collect()
}

/// Returns the name of the only operation `reference` points to, panicking if there isn't exactly one.
fn resolve(batches_info: &[BatchInfo], reference: OccurrenceRef) -> String {
    let mut names = Vec::new();
    for batch in batches_info {
        for system in batch.systems() {
            if reference == OccurrenceRef::System(system.occurrence.id) {
                names.push(system.name.clone());
            }
        }
        for regroup in &batch.regroups {
            if reference == OccurrenceRef::Regroup(regroup.occurrence.id) {
                names.push(format!("regroup{}", regroup.occurrence.id));
            }
        }
    }

    let [name] = &names[..] else {
        panic!("{reference:?} points to {names:?}");
    };

    name.clone()
}

/// Returns the name of each predecessor of the system whose occurrence id is `id`, with its origins.
fn resolved_after(batches_info: &[BatchInfo], id: usize) -> Vec<(String, Vec<OrderingOrigin>)> {
    let (_, occurrence) = system_occurrence(batches_info, id);

    occurrence
        .after
        .iter()
        .map(|predecessor| {
            (
                resolve(batches_info, predecessor.occurrence),
                predecessor.origins.clone(),
            )
        })
        .collect()
}

fn offset_1() {}

fn offset_2() {}

fn early() {}

fn late() {}

/// Asserts that references resolve to the intended occurrences when callables registered earlier
/// offset the callable indices, and that the schedule keeps the declared order.
///
/// The workload `""` is `write_a`, `read_a`, `late.after_all(write_a)` and `early.before_all(write_a)`,
/// registered after two unrelated systems that shift every callable index by 2 from its position.
#[test]
fn occurrence_ids_offset() {
    let world = abc_world();
    Workload::new("offset")
        .with_system(offset_1)
        .with_system(offset_2)
        .add_to_world(&world)
        .unwrap();
    Workload::new("")
        .with_system(write_a)
        .with_system(read_a)
        .with_system(late.after_all(write_a))
        .with_system(early.before_all(write_a))
        .add_to_world(&world)
        .unwrap();
    let batches_info = &world.workloads_info().0[""].batches_info;

    assert_eq!(
        Schedule::registered(&world, "").parallel,
        vec![
            steps(&["early", "regroup0"]),
            steps(&["write_a"]),
            steps(&["regroup1", "late"]),
            steps(&["read_a"]),
        ]
    );

    let early_name = format!("{:?}", early.as_label());
    let write_a_name = format!("{:?}", write_a.as_label());
    let (writer, _) = system_occurrence(batches_info, 0);
    assert_eq!(writer.name, write_a_name);
    assert_ne!(writer.unique_id, 0);
    assert_eq!(
        resolved_after(batches_info, 0),
        [
            (
                early_name,
                vec![OrderingOrigin::BeforeAll(write_a_name.clone())]
            ),
            ("regroup0".to_string(), vec![OrderingOrigin::Regroup]),
        ]
    );
    assert_eq!(
        resolved_after(batches_info, 1),
        [
            (write_a_name.clone(), vec![OrderingOrigin::Implicit]),
            ("regroup0".to_string(), vec![OrderingOrigin::Regroup]),
            ("regroup1".to_string(), vec![OrderingOrigin::Regroup]),
        ]
    );
    assert_eq!(
        resolved_after(batches_info, 2),
        [(
            write_a_name.clone(),
            vec![OrderingOrigin::AfterAll(write_a_name.clone())]
        )]
    );
}

/// Writes `Usize`.
fn write(_: ViewMut<'_, Usize>) {}

/// Reads `Usize`.
fn read(_: View<'_, Usize>) {}

/// Asserts that two occurrences of one callable share their `unique_id` but get their own
/// occurrence id, that later ones point to the exact earlier occurrences, and that they run in
/// the declared order, one per batch.
#[test]
fn occurrence_ids_repeated() {
    let repeated = || {
        Workload::new("")
            .with_system(write)
            .with_system(write)
            .with_system(read)
    };
    let (_, info) = repeated().build().unwrap();
    let batches_info = &info.batches_info;

    assert_eq!(
        Schedule::built(repeated()).parallel,
        vec![steps(&["write"]), steps(&["write"]), steps(&["read"])]
    );

    let (first, _) = system_occurrence(batches_info, 0);
    let (second, _) = system_occurrence(batches_info, 1);
    assert_eq!(first.unique_id, second.unique_id);
    assert!(after_refs(batches_info, 0).is_empty());
    assert_eq!(after_refs(batches_info, 1), [OccurrenceRef::System(0)]);
    assert_eq!(
        after_refs(batches_info, 2),
        [OccurrenceRef::System(0), OccurrenceRef::System(1)]
    );
    assert_eq!(resolve(batches_info, OccurrenceRef::System(1)), first.name);
}

/// Asserts that a `before_all` edge names its label as origin, on the system running later.
#[test]
fn before_all_origin() {
    fn sys_a() {}
    fn sys_b() {}

    let (_, info) = Workload::new("")
        .with_system(sys_a)
        .with_system(sys_b.before_all(sys_a))
        .build()
        .unwrap();

    let (_, occurrence) = system_occurrence(&info.batches_info, 0);
    assert_eq!(
        occurrence.after,
        [Predecessor {
            occurrence: OccurrenceRef::System(1),
            origins: vec![OrderingOrigin::BeforeAll(format!("{:?}", sys_a.as_label()))],
        }]
    );
}

/// Asserts that an `after_all` edge names its label as origin.
#[test]
fn after_all_origin() {
    fn sys_a() {}
    fn sys_b() {}

    let (_, info) = Workload::new("")
        .with_system(sys_a.after_all(sys_b))
        .with_system(sys_b)
        .build()
        .unwrap();

    let (_, occurrence) = system_occurrence(&info.batches_info, 0);
    assert_eq!(
        occurrence.after,
        [Predecessor {
            occurrence: OccurrenceRef::System(1),
            origins: vec![OrderingOrigin::AfterAll(format!("{:?}", sys_b.as_label()))],
        }]
    );
}

/// Asserts that an edge with two reasons, a borrow conflict and a barrier, lists both origins once.
#[test]
fn several_origins() {
    let (_, info) = Workload::new("")
        .with_system(write_a)
        .with_barrier()
        .with_system(read_a)
        .build()
        .unwrap();

    let (_, occurrence) = system_occurrence(&info.batches_info, 1);
    assert_eq!(
        occurrence.after,
        [Predecessor {
            occurrence: OccurrenceRef::System(0),
            origins: vec![OrderingOrigin::Implicit, OrderingOrigin::Barrier],
        }]
    );
}

/// Asserts that a barrier alone, without a conflict, is the only origin.
#[test]
fn barrier_origin() {
    fn sys_a() {}
    fn sys_b() {}

    let (_, info) = Workload::new("")
        .with_system(sys_a)
        .with_barrier()
        .with_system(sys_b)
        .build()
        .unwrap();

    let (_, occurrence) = system_occurrence(&info.batches_info, 1);
    assert_eq!(occurrence.after[0].origins, [OrderingOrigin::Barrier]);
}

/// Asserts that a sequential workload's legacy `before_all` names the next child by its position
/// only, so the export doesn't depend on the sequential conversions made before.
#[test]
fn sequential_before_all() {
    fn sys_a() {}
    fn sys_b() {}

    let _earlier = (sys_a, sys_b).into_sequential_workload();
    let (_, info) = (sys_a, sys_b)
        .into_sequential_workload()
        .rename("")
        .build()
        .unwrap();

    let (system, _) = system_occurrence(&info.batches_info, 0);
    assert_eq!(system.before_all.len(), 1);
    assert_eq!(system.before_all[0].constraint, "Sequential(child 1)");
}

/// Asserts that consecutive children of a sequential workload are linked with a sequential origin,
/// not their private label.
#[test]
fn sequential_origin() {
    fn sys_a() {}
    fn sys_b() {}

    let (_, info) = (sys_a, sys_b)
        .into_sequential_workload()
        .rename("")
        .build()
        .unwrap();

    let (_, occurrence) = system_occurrence(&info.batches_info, 1);
    assert_eq!(
        occurrence.after,
        [Predecessor {
            occurrence: OccurrenceRef::System(0),
            origins: vec![OrderingOrigin::Sequential],
        }]
    );
}

/// Asserts that what a system's condition borrows is exported apart from the system's own borrows,
/// and that the footprint has both.
#[test]
fn condition_footprint() {
    let (_, info) = Workload::new("")
        .with_system(write_a.run_if(|_: View<'_, B>| true))
        .build()
        .unwrap();

    let (system, occurrence) = system_occurrence(&info.batches_info, 0);
    assert_eq!(system.borrow, [type_info::<A>(Mutability::Exclusive)]);
    assert_eq!(occurrence.conditions, [type_info::<B>(Mutability::Shared)]);
    assert_eq!(
        occurrence.footprint,
        [
            type_info::<A>(Mutability::Exclusive),
            type_info::<B>(Mutability::Shared)
        ]
    );
    assert!(!occurrence.thread_bound);
}

/// Asserts that a system whose condition conflicts with an earlier system is ordered after it with
/// an implicit origin, even though the system itself borrows nothing.
#[test]
fn condition_ordering() {
    fn noop() {}

    let (_, info) = Workload::new("")
        .with_system(write_a)
        .with_system(noop.run_if(|_: View<'_, A>| true))
        .build()
        .unwrap();

    let (system, occurrence) = system_occurrence(&info.batches_info, 1);
    assert!(system.borrow.is_empty());
    assert_eq!(
        occurrence.after,
        [Predecessor {
            occurrence: OccurrenceRef::System(0),
            origins: vec![OrderingOrigin::Implicit],
        }]
    );
}

/// Asserts that a regroup comes after a system whose condition reads its set, with a `Borrows`
/// origin. The condition doesn't trigger the regroup, the later `read_a` does.
#[test]
fn condition_regroup_ordering() {
    fn noop() {}

    let world = abc_world();
    let workload = Workload::new("")
        .with_system(noop.run_if(|_: View<'_, B>| true))
        .with_system(read_a);

    let batches_info = registered_batches(workload, &world);

    let regroups = regroups(&batches_info);
    assert_eq!(regroups.len(), 1);
    assert_eq!(
        regroups[0].occurrence.after,
        [Predecessor {
            occurrence: OccurrenceRef::System(0),
            origins: vec![OrderingOrigin::Borrows],
        }]
    );
}

/// Asserts that a system borrowing `AllStorages` is exported as running on the calling thread.
#[test]
fn thread_bound_system() {
    fn all(_: crate::views::AllStoragesViewMut<'_>) {}

    let (_, info) = Workload::new("").with_system(all).build().unwrap();

    let (_, occurrence) = system_occurrence(&info.batches_info, 0);
    assert!(occurrence.thread_bound);
}

/// Asserts that repeated regroups of one set get distinct ids, each pending regroup pointing to the
/// exact occurrence that wrote before it with `Writes`, and to the other earlier steps borrowing A.
/// Each reader points to every earlier regroup of A.
#[test]
fn regroup_occurrences() {
    let world = abc_world();
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(read_a)
        .with_system(write_a)
        .with_system(read_a);

    let batches_info = registered_batches(workload, &world);

    let regroups = regroups(&batches_info);
    let triggers: Vec<_> = regroups.iter().map(|regroup| regroup.trigger).collect();
    assert_eq!(
        triggers,
        [
            RegroupTrigger::Entry,
            RegroupTrigger::Pending,
            RegroupTrigger::Pending
        ]
    );
    let occurrences: Vec<&RegroupOccurrence> =
        regroups.iter().map(|regroup| &regroup.occurrence).collect();
    let ids: Vec<usize> = occurrences.iter().map(|occurrence| occurrence.id).collect();
    assert!(ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2]);

    let predecessors = |index: usize| -> Vec<(OccurrenceRef, &[OrderingOrigin])> {
        occurrences[index]
            .after
            .iter()
            .map(|predecessor| (predecessor.occurrence, &*predecessor.origins))
            .collect()
    };
    assert!(predecessors(0).is_empty());
    assert_eq!(
        predecessors(1),
        [
            (OccurrenceRef::System(0), &[OrderingOrigin::Writes][..]),
            (OccurrenceRef::Regroup(ids[0]), &[OrderingOrigin::Regroup]),
        ]
    );
    // The first `write_a` was taken care of by the previous regroup, it only borrows A now.
    assert_eq!(
        predecessors(2),
        [
            (OccurrenceRef::System(0), &[OrderingOrigin::Borrows][..]),
            (OccurrenceRef::System(1), &[OrderingOrigin::Borrows]),
            (OccurrenceRef::System(2), &[OrderingOrigin::Writes]),
            (OccurrenceRef::Regroup(ids[0]), &[OrderingOrigin::Regroup]),
            (OccurrenceRef::Regroup(ids[1]), &[OrderingOrigin::Regroup]),
        ]
    );

    assert_eq!(
        after_refs(&batches_info, 0),
        [OccurrenceRef::Regroup(ids[0])]
    );
    assert_eq!(
        after_refs(&batches_info, 1),
        [
            OccurrenceRef::System(0),
            OccurrenceRef::Regroup(ids[0]),
            OccurrenceRef::Regroup(ids[1])
        ]
    );
    assert_eq!(
        after_refs(&batches_info, 3),
        [
            OccurrenceRef::System(0),
            OccurrenceRef::System(2),
            OccurrenceRef::Regroup(ids[0]),
            OccurrenceRef::Regroup(ids[1]),
            OccurrenceRef::Regroup(ids[2])
        ]
    );
}

/// Asserts that a system's conflict with an occurrence further back is replaced by the regroup it
/// waits for, even when another occurrence of the same callable is in the previous batch.
///
/// The reader conflicts with the first `noop`, through its condition. The second `noop` waits for
/// `write_a` like the pending regroup, so they share the batch before the reader.
#[test]
fn regroup_conflict_repeated() {
    fn noop() {}
    fn read_a_usize(_: View<'_, A>, _: View<'_, Usize>) {}

    let world = abc_world();
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(noop.run_if(|_: ViewMut<'_, Usize>| true))
        .with_system(noop.after_all(write_a))
        .with_system(read_a_usize);

    let batches_info = registered_batches(workload, &world);

    let (first_noop, _) = system_occurrence(&batches_info, 1);
    let (second_noop, _) = system_occurrence(&batches_info, 2);
    assert_eq!(first_noop.unique_id, second_noop.unique_id);
    let previous = &batches_info[batches_info.len() - 2];
    assert!(previous.systems().any(|system| system.occurrence.id == 2));
    assert!(previous.systems().all(|system| system.occurrence.id != 1));
    assert_eq!(previous.regroups.len(), 1);

    let (reader, _) = system_occurrence(&batches_info, 3);
    assert_eq!(
        reader.conflict,
        Some(Conflict::Regroup(type_info::<A>(Mutability::Shared)))
    );
}

/// Asserts that a regroup's exported affinity follows the slot it was placed in.
#[cfg(feature = "thread_local")]
#[test]
fn thread_bound_regroup() {
    use crate::borrow::NonSend;

    let world = World::builder_for_test()
        .with_group::<(A, NonSend<B>)>()
        .build();

    let batches_info = registered_batches(Workload::new("").with_system(read_a), &world);

    let regroups = regroups(&batches_info);
    assert!(regroups[0].occurrence.thread_bound);
}

/// Asserts that occurrence metadata without facts, no conditions, footprint or predecessors,
/// goes through serde JSON as empty lists and comes back the same.
#[cfg(feature = "serde1")]
#[test]
fn serde_empty_facts() {
    fn noop() {}

    let (_, info) = Workload::new("").with_system(noop).build().unwrap();

    let json = serde_json::to_string(&info).unwrap();
    let deserialized: WorkloadInfo = serde_json::from_str(&json).unwrap();

    assert!(json.contains(r#""conditions":[],"footprint":[],"thread_bound":false,"after":[]"#));
    // Nothing holds a `TypeInfo`, whose storage id serde skips, so the whole information compares.
    assert_eq!(deserialized, info);
}

/// Asserts that the occurrence ids and references of a registered workload survive serde JSON.
#[cfg(feature = "serde1")]
#[test]
fn serde_occurrences() {
    let world = abc_world();
    let workload = Workload::new("")
        .with_system(write_a)
        .with_system(read_a)
        .with_system(write_a);
    let batches_info = registered_batches(workload, &world);

    let json = serde_json::to_string(&batches_info).unwrap();
    let deserialized: Vec<BatchInfo> = serde_json::from_str(&json).unwrap();

    // `TypeInfo` equality uses the storage id serde skips, so we compare what doesn't contain one.
    for id in 0..3 {
        assert_eq!(after_refs(&deserialized, id), after_refs(&batches_info, id));
    }
    let regroup_facts = |batches_info: &[BatchInfo]| -> Vec<RegroupOccurrence> {
        regroups(batches_info)
            .into_iter()
            .map(|regroup| regroup.occurrence.clone())
            .collect()
    };
    assert_eq!(regroup_facts(&deserialized), regroup_facts(&batches_info));
}
