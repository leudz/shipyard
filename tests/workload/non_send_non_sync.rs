use shipyard::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn basic() {
    #[system(Push)]
    fn run(vecs: &Rc<RefCell<Vec<u32>>>) {
        vecs.iter().next().unwrap().borrow_mut().push(0);
    }

    let world = World::default();
    world.register_non_send_non_sync::<(Rc<RefCell<Vec<u32>>>,)>();
    world.run::<(EntitiesMut, &mut Rc<RefCell<Vec<u32>>>), _, _>(|(mut entities, mut vecs)| {
        entities.add_entity(&mut vecs, Rc::new(RefCell::new(Vec::new())));
    });
    world.add_workload::<Push, _>("Push");
    world.run_default();

    world.run::<&Rc<RefCell<Vec<u32>>>, _, _>(|vecs| {
        assert_eq!(&**vecs.iter().next().unwrap().borrow(), &[0][..]);
    });
}
