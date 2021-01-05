use shipyard::*;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn basic() {
    fn push(vecs: NonSendSync<View<Rc<RefCell<Vec<u32>>>>>) {
        vecs.iter().next().unwrap().borrow_mut().push(0);
    }

    let world = World::default();

    world
        .run(
            |mut entities: EntitiesViewMut,
             mut vecs: NonSendSync<ViewMut<Rc<RefCell<Vec<u32>>>>>| {
                entities.add_entity(&mut *vecs, Rc::new(RefCell::new(Vec::new())));
            },
        )
        .unwrap();

    Workload::builder("Push")
        .with_system(system!(push))
        .add_to_world(&world)
        .unwrap();
    world.run_default().unwrap();

    world
        .run(|vecs: NonSendSync<ViewMut<Rc<RefCell<Vec<u32>>>>>| {
            assert_eq!(&**vecs.iter().next().unwrap().borrow(), &[0][..]);
        })
        .unwrap();
}
