use super::*;

#[test]
fn skip_if_missing_storage() {
    let world = World::new_for_test();

    Workload::new("test")
        .skip_if_storage_empty::<Usize>()
        .with_system(|| panic!())
        .build()
        .unwrap()
        .0
        .run_with_world(&world)
        .unwrap();

    Workload::new("test")
        .skip_if_storage_empty::<Usize>()
        .with_system(|| panic!())
        .add_to_world(&world)
        .unwrap();

    world.run_default_workload().unwrap();
}

#[test]
fn system_skip_if_missing_storage() {
    let world = World::new_for_test();

    Workload::new("test")
        .with_system((|| -> () { panic!() }).skip_if_storage_empty::<Usize>())
        .build()
        .unwrap()
        .0
        .run_with_world(&world)
        .unwrap();

    Workload::new("test")
        .with_system((|| -> () { panic!() }).skip_if_storage_empty::<Usize>())
        .add_to_world(&world)
        .unwrap();

    world.run_default_workload().unwrap();
}

#[test]
fn skip_if_empty_storage() {
    let mut world = World::new_for_test();

    let eid = world.add_entity((Usize(0),));
    world.remove::<(Usize,)>(eid);

    Workload::new("test")
        .skip_if_storage_empty::<Usize>()
        .with_system(|| -> () { panic!() })
        .build()
        .unwrap()
        .0
        .run_with_world(&world)
        .unwrap();

    Workload::new("test")
        .skip_if_storage_empty::<Usize>()
        .with_system(|| -> () { panic!() })
        .add_to_world(&world)
        .unwrap();

    world.run_default_workload().unwrap();
}

#[test]
fn system_skip_if_empty_storage() {
    let mut world = World::new_for_test();

    let eid = world.add_entity((Usize(0),));
    world.remove::<(Usize,)>(eid);

    Workload::new("test")
        .with_system((|| -> () { panic!() }).skip_if_storage_empty::<Usize>())
        .build()
        .unwrap()
        .0
        .run_with_world(&world)
        .unwrap();

    Workload::new("test")
        .with_system((|| -> () { panic!() }).skip_if_storage_empty::<Usize>())
        .add_to_world(&world)
        .unwrap();

    world.run_default_workload().unwrap();
}

#[test]
fn workload_merge_skip_if() {
    let world = World::new_for_test();

    world.add_unique(Usize(0));

    world.add_workload(|| {
        (
            (|| -> () { panic!() })
                .into_workload()
                .skip_if_missing_unique::<U32>(),
            (|mut u: UniqueViewMut<'_, Usize>| u.0 += 1).into_workload(),
        )
    });

    world.run_default_workload().unwrap();

    assert_eq!(world.borrow::<UniqueView<'_, Usize>>().unwrap().0, 1);
}

#[test]
fn try_system_run_if() {
    fn try_sys() -> Result<(), error::MissingComponent> {
        Err(error::MissingComponent {
            id: crate::EntityId::dead(),
            name: "",
        })
    }

    let (workload, _) = Workload::new("")
        .with_try_system(try_sys.into_workload_try_system().unwrap().run_if(|| true))
        .build()
        .unwrap();

    let world = World::new_for_test();

    assert!(
        workload
            .run_with_world(&world)
            .err()
            .unwrap()
            .custom_error()
            .is_some()
    );
}
