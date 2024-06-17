use shipyard::{Borrow, BorrowInfo, Component, EntitiesViewMut, IntoIter, ViewMut, World};

#[test]
#[rustfmt::skip]
#[allow(unused)]
fn view() {
#[derive(Component)]
struct Parent;
#[derive(Component)]
struct Child;

// ANCHOR: into_iter
#[derive(Borrow, BorrowInfo, IntoIter)]
#[shipyard(item_name = "Node")]
struct Hierarchy<'v> {
    #[shipyard(item_field_skip)]
    entities: EntitiesViewMut<'v>,
    #[shipyard(item_field_name = "parent")]
    parents: ViewMut<'v, Parent>,
    #[shipyard(item_field_name = "child")]
    children: ViewMut<'v, Child>,
}

let world = World::new();

world.run(|mut hierarchy: Hierarchy| {
    for Node { parent, child } in hierarchy.iter() {
    }
});
// ANCHOR_END: into_iter
}
