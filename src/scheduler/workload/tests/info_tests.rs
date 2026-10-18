use super::super::*;
use crate::borrow::Mutability;
use crate::scheduler::info::{BeforeAfterConstraint, Conflict, SystemInfo};
use crate::scheduler::system_modificator::SystemModificator;
use crate::sparse_set::SparseSet;
use crate::views::{View, ViewMut};
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
