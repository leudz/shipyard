use shipyard::*;
use std::cell::RefCell;
use std::rc::Rc;

struct MyRc(Rc<RefCell<Vec<u32>>>);
impl Component for MyRc {
    type Tracking = track::Nothing;
}

#[test]
fn basic() {
    fn push(vecs: NonSendSync<View<MyRc>>) {
        vecs.iter().next().unwrap().0.borrow_mut().push(0);
    }

    let world = World::default();

    world
        .run(
            |mut entities: EntitiesViewMut, mut vecs: NonSendSync<ViewMut<MyRc>>| {
                entities.add_entity(&mut *vecs, MyRc(Rc::new(RefCell::new(Vec::new()))));
            },
        )
        .unwrap();

    Workload::builder("Push")
        .with_system(push)
        .add_to_world(&world)
        .unwrap();
    world.run_default().unwrap();

    world
        .run(|vecs: NonSendSync<ViewMut<MyRc>>| {
            assert_eq!(&**vecs.iter().next().unwrap().0.borrow(), &[0][..]);
        })
        .unwrap();
}
