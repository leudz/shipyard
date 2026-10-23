use super::super::*;
use super::pending::{read_a, write_a};
use super::{A, B, C};
use crate::borrow::Mutability;
use crate::scheduler::info::{
    BatchInfo, BeforeAfterConstraint, Conflict, RegroupInfo, RegroupTrigger, SystemInfo,
};
use crate::scheduler::system_modificator::SystemModificator;
use crate::sparse_set::SparseSet;
use crate::storage::StorageId;
use crate::views::{View, ViewMut};
use crate::world::World;
use alloc::format;
use alloc::string::ToString;

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
        systems0.next(),
        Some(&SystemInfo {
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
            unique_id: 1
        })
    );
    assert_eq!(systems0.next(), None);

    let mut systems1 = info.batches_info[1].systems();
    assert_eq!(
        systems1.next(),
        Some(&SystemInfo {
            name: "System(shipyard::scheduler::workload::tests::info_tests::before_all::sys_a)"
                .to_string(),
            borrow: vec![],
            conflict: None,
            after: vec![1],
            after_all: vec![],
            before_all: vec![],
            unique_id: 0
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
        systems0.next(),
        Some(&SystemInfo {
            name: "System(shipyard::scheduler::workload::tests::info_tests::after_all::sys_b)"
                .to_string(),
            borrow: vec![],
            conflict: None,
            after: vec![],
            after_all: vec![],
            before_all: vec![],
            unique_id: 1
        })
    );
    assert_eq!(systems0.next(), None);

    let mut systems1 = info.batches_info[1].systems();
    assert_eq!(
        systems1.next(),
        Some(&SystemInfo {
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
            unique_id: 0
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
        systems0.next(),
        Some(&SystemInfo {
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
            unique_id: 0
        })
    );

    let mut systems1 = info.batches_info[1].systems();
    assert_eq!(
        systems1.next(),
        Some(&SystemInfo {
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
            unique_id: 1
        })
    );

    let mut systems2 = info.batches_info[2].systems();
    assert_eq!(
        systems2.next(),
        Some(&SystemInfo {
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
            unique_id: 2
        })
    );
    assert_eq!(
        systems2.next(),
        Some(&SystemInfo {
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
            unique_id: 3
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

/// Asserts that information serialized before regroups existed still deserializes,
/// without regroups.
#[cfg(feature = "serde1")]
#[test]
fn serde_without_regroups() {
    let (_, info) = Workload::new("").with_system(read_a).build().unwrap();
    let json = serde_json::to_string(&info).unwrap();
    let old_json = json.replace(",\"regroups\":[]", "");
    assert_ne!(old_json, json);

    let deserialized: WorkloadInfo = serde_json::from_str(&old_json).unwrap();

    assert!(deserialized.batches_info[0].regroups.is_empty());
    assert_eq!(serde_json::to_string(&deserialized).unwrap(), json);
}
