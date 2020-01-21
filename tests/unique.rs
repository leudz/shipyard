use shipyard::prelude::*;

#[test]
fn unique_storage() {
    let world = World::default();
    world.add_unique(0usize);

    world.run::<Unique<&mut usize>, _, _>(|mut x| {
        *x += 1;
    });
    world.run::<Unique<&usize>, _, _>(|x| {
        assert_eq!(*x, 1);
    });
}

#[test]
fn not_unique_storage() {
    match std::panic::catch_unwind(|| {
        let world = World::new();

        world.run::<Unique<&usize>, _, _>(|x| {
            assert_eq!(*x, 1);
        });
    }) {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            format!("{}", err.downcast::<String>().unwrap()),
            "called `Result::unwrap()` on an `Err` value: usize's storage isn't unique.\n\
            You might have forgotten to declare it, replace world.register::<usize>() by world.register_unique(/* your_storage */).\n\
            If it isn't supposed to be a unique storage, replace Unique<&usize> by &usize."
        ),
    }

    match std::panic::catch_unwind(|| {
        let world = World::new();

        world.run::<Unique<&mut usize>, _, _>(|x| {
            assert_eq!(*x, 1);
        });
    }) {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            format!("{}", err.downcast::<String>().unwrap()),
            "called `Result::unwrap()` on an `Err` value: usize's storage isn't unique.\n\
            You might have forgotten to declare it, replace world.register::<usize>() by world.register_unique(/* your_storage */).\n\
            If it isn't supposed to be a unique storage, replace Unique<&mut usize> by &mut usize."
        ),
    }
}
