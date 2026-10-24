use shipyard::world::WorldBuilder;
use shipyard::{Component, World};

#[derive(Component)]
struct Transform;
#[derive(Component)]
struct Sprite;
#[derive(Component)]
struct SpriteAnimation;
#[derive(Component)]
struct Skeleton;
#[derive(Component)]
struct AnimationPlayer;
#[derive(Component)]
struct Health;
#[derive(Component)]
struct Poisoned;

#[test]
fn declaration() {
    // ANCHOR: declaration
    let world = World::builder()
        .with_group::<(Sprite, Transform)>()
        .with_group::<(Sprite, SpriteAnimation)>()
        .with_group::<(Skeleton, AnimationPlayer)>()
        .build();
    // ANCHOR_END: declaration
}

#[test]
fn iter() {
    let world = World::builder().with_group::<(Sprite, Transform)>().build();

    // ANCHOR: iter
    for (sprite, transform) in world.iter::<(&Sprite, &Transform)>().iter() {
        // -- snip --
    }
    // ANCHOR_END: iter
}
