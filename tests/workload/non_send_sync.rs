use shipyard::*;
use std::cell::RefCell;
use std::rc::Rc;

struct MyRc(Rc<RefCell<Vec<u32>>>);
impl Component for MyRc {
    type Tracking = track::Untracked;
}

struct NotSend(*const ());
impl Component for NotSend {
    type Tracking = track::Untracked;
}
unsafe impl Sync for NotSend {}

struct NotSync(*const ());
impl Component for NotSync {
    type Tracking = track::Untracked;
}
unsafe impl Send for NotSync {}

#[test]
fn basic() {
    fn push(vecs: NonSendSync<View<MyRc>>) {
        vecs.iter().next().unwrap().0.borrow_mut().push(0);
    }

    let world = World::default();

    world.run(
        |mut entities: EntitiesViewMut, mut vecs: NonSendSync<ViewMut<MyRc>>| {
            entities.add_entity(&mut *vecs, MyRc(Rc::new(RefCell::new(Vec::new()))));
        },
    );

    Workload::new("Push")
        .with_system(push)
        .add_to_world(&world)
        .unwrap();
    world.run_default_workload().unwrap();

    world.run(|vecs: NonSendSync<ViewMut<MyRc>>| {
        assert_eq!(&**vecs.iter().next().unwrap().0.borrow(), &[0][..]);
    });
}

#[test]
fn tracking_enabled() {
    fn w() -> Workload {
        (
            |_: NonSend<View<NotSend, track::All>>| {},
            |_: NonSend<ViewMut<NotSend, track::All>>| {},
            |_: NonSync<View<NotSync, track::All>>| {},
            |_: NonSync<ViewMut<NotSync, track::All>>| {},
            |_: NonSendSync<View<MyRc, track::All>>| {},
            |_: NonSendSync<ViewMut<MyRc, track::All>>| {},
        )
            .into_workload()
    }

    let world = World::new();

    world.add_workload(w);

    world.run_workload(w).unwrap();
}
