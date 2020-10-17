use shipyard::*;

#[test]
fn simple_sort() {
    let world = World::new();

    world
        .try_run(
            |(mut entities, mut usizes): (EntitiesViewMut, ViewMut<usize>)| {
                entities.add_entity(&mut usizes, 5);
                entities.add_entity(&mut usizes, 2);
                entities.add_entity(&mut usizes, 4);
                entities.add_entity(&mut usizes, 3);
                entities.add_entity(&mut usizes, 1);

                usizes.sort().unstable(Ord::cmp);

                let mut prev = 0;
                (&mut usizes).iter().for_each(|x| {
                    assert!(prev <= *x);
                    prev = *x;
                });
            },
        )
        .unwrap();
}
