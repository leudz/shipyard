use shipyard::*;
use std::rc::Rc;

#[allow(unused)]
// ANCHOR: non_send_sync
fn run(rcs_usize: NonSendSync<View<Rc<usize>>>, rc_u32: NonSendSync<UniqueView<Rc<u32>>>) {}
// ANCHOR_END: non_send_sync

#[test]
fn test() {
    let _ = World::new().run(run);
}
