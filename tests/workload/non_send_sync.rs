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
        .try_run(
            |mut entities: EntitiesViewMut,
             mut vecs: NonSendSync<ViewMut<Rc<RefCell<Vec<u32>>>>>| {
                entities.add_entity(&mut *vecs, Rc::new(RefCell::new(Vec::new())));
            },
        )
        .unwrap();

    world
        .try_add_workload("Push")
        .unwrap()
        .with_system(system!(push))
        .build();
    world.try_run_default().unwrap();

    world
        .try_run(|vecs: NonSendSync<ViewMut<Rc<RefCell<Vec<u32>>>>>| {
            assert_eq!(&**vecs.iter().next().unwrap().borrow(), &[0][..]);
        })
        .unwrap();
}
