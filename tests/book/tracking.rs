use shipyard::{track, AddComponent, Component, IntoIter, View, ViewMut};

// ANCHOR: component
struct Life(f32);
impl Component for Life {
    type Tracking = track::Modification;
}
// ANCHOR_END: component

#[rustfmt::skip]
mod proc {
    use shipyard::Component;

// ANCHOR: component_proc

// or with the proc macro

#[derive(Component)]
#[track(Modification)]
struct Life(f32);
// ANCHOR_END: component_proc
}

#[derive(Component)]
struct IsDead;

// ANCHOR: run
fn run(life: View<Life>, mut is_dead: ViewMut<IsDead>) {
    for (entity, life) in life.modified().iter().with_id() {
        if life.0 <= 0.0 {
            is_dead.add_component_unchecked(entity, IsDead);
        }
    }
}
// ANCHOR_END: run
