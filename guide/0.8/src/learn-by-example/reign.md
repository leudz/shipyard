# Reign

We've had plenty of time to think of a way for our `Player` to get back at those pesky `Friends`.\
Sometimes, the simplest solution is the best.\
If the `Friends` can overpower the `Player` when they are fully grown, we shouldn't let them reach that size.\
I'm sure the `Player` can overcome `Friend` that are smaller than them.

```rust,noplaypen
use shipyard::{
    Component, EntitiesView, IntoIter, IntoWithId, Unique, UniqueView, UniqueViewMut, View,
    ViewMut, World,
};

#[derive(Component)]
struct ToDelete;

fn collision(
    entities: EntitiesView,
    mut player: UniqueViewMut<Player>,
    v_friend: View<Friend>,
    mut vm_to_delete: ViewMut<ToDelete>,
) -> Result<(), GameOver> {
    for (eid, friend) in v_friend.iter().with_id() {
        if friend.0.size == MAX_SIZE && friend.0.collide(&player.square) {
            // -- SNIP --
        } else if player.square.size >= friend.0.size && player.square.collide(&friend.0) {
            player.square.size = (player.square.size + INIT_SIZE / 2.).min(MAX_SIZE - 0.01);
            entities.add_component(eid, &mut vm_to_delete, ToDelete);
        }
    }

    Ok(())
}
```

It appears our `Player` can even overcome `Friends` of equal size.\
By... eating them!?

Remember when we added `Friends` to the [`World`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html), each one was assigned an [`EntityId`](https://docs.rs/shipyard/0.8/shipyard/struct.EntityId.html).\
We can iterate over both components and the [`EntityId`](https://docs.rs/shipyard/0.8/shipyard/struct.EntityId.html) of the entity that owns them by using `with_id`.

Then we can use this [`EntityId`](https://docs.rs/shipyard/0.8/shipyard/struct.EntityId.html) to add another component to the vanquished `Friends`.\
As you may have noticed we are not modifying `entities`. We only need it to check that the `eid` is alive.

`ToDelete` is not a special component, we still have to make it do its job.

```rust,noplaypen
use shipyard::{
    AllStoragesViewMut, Component, EntitiesView, IntoIter, IntoWithId, SparseSet, Unique,
    UniqueView, UniqueViewMut, View, ViewMut, World,
};

async fn main() {
    // -- SNIP --

    loop {
        clear_background(WHITE);

        world.run(move_player);
        world.run(move_friends);
        world.run(grow);
        if world.run(collision).is_err() {
            panic!("Murder");
        }
        world.run(clean_up);
        world.run(render);

        next_frame().await
    }
}

fn clean_up(mut all_storages: AllStoragesViewMut) {
    all_storages.delete_any::<SparseSet<ToDelete>>();
}
```

[`AllStorages`](https://docs.rs/shipyard/0.8/shipyard/struct.AllStorages.html) is the part of [`World`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html) that stores all components and entities.\
We are using it to [`delete_any`](https://docs.rs/shipyard/0.8/shipyard/struct.AllStorages.html#method.delete_any) entity that has a `ToDelete` component in a [`SparseSet`](https://docs.rs/shipyard/0.8/shipyard/struct.SparseSet.html) storage.\
[`SparseSet`](https://docs.rs/shipyard/0.8/shipyard/struct.SparseSet.html) is the storage for all [`Component`](https://docs.rs/shipyard/0.8/shipyard/trait.Component.html)s. [`Unique`](https://docs.rs/shipyard/0.8/shipyard/trait.Unique.html)s have a different storage and you can add custom storages to the [`World`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html) but that's an advanced feature.

## It's over

Defeating smaller `Friends` is nice but most of the time they've grown by the time the `Player` reaches them.\
The `Player` needs more power.

```rust,noplaypen
use shipyard::{
    AllStoragesViewMut, Component, EntitiesView, EntitiesViewMut, IntoIter, IntoWithId, SparseSet,
    Unique, UniqueView, UniqueViewMut, View, ViewMut, World,
};

const POWER_PELLET_SPAWN_RATE: u32 = 150;

async fn main() {
    // -- SNIP --

    let player = Player {
        square: Square {
            x,
            y,
            size: INIT_SIZE * 3.0,
        },
        pellet_counter: 0,
    };

    // -- SNIP --

    loop {
        // -- SNIP --

        world.run(grow);
        world.run(counters);
        world.run(spawn);
        if world.run(collision).is_err() {
            panic!("Murder");
        }

        // -- SNIP --
    }
}

struct Player {
    square: Square,
    pellet_counter: u32,
}

impl Player {
    fn power_up(&mut self) {
        self.pellet_counter = 120;
    }

    fn is_powered_up(&self) -> bool {
        self.pellet_counter > 0
    }
}

#[derive(Component)]
struct PowerPellet(Square);

fn render(player: UniqueView<Player>, v_friend: View<Friend>, v_power_pellets: View<PowerPellet>) {
    for pellet in v_power_pellets.iter() {
        pellet.0.render(YELLOW);
    }

    // -- SNIP --

    if player.is_powered_up() {
        player.square.render(YELLOW);
    } else {
        player.square.render(BLUE);
    }
}


fn collision(
    entities: EntitiesView,
    mut player: UniqueViewMut<Player>,
    v_friend: View<Friend>,
    v_power_pellets: View<PowerPellet>,
    mut vm_to_delete: ViewMut<ToDelete>,
) -> Result<(), GameOver> {
    for (eid, pellet) in v_power_pellets.iter().with_id() {
        if player.square.collide(&pellet.0) {
            player.power_up();
            entities.add_component(eid, &mut vm_to_delete, ToDelete);
        }
    }

    for (eid, friend) in v_friend.iter().with_id() {
        if friend.0.size == MAX_SIZE && friend.0.collide(&player.square) {
            if player.is_powered_up() {
                player.square.size = (player.square.size + INIT_SIZE / 2.).min(MAX_SIZE - 0.01);
                entities.add_component(eid, &mut vm_to_delete, ToDelete);

                continue;
            }

            player.square.size -= INIT_SIZE / 2.;

            // -- SNIP --
        } else if player.square.size >= friend.0.size && player.square.collide(&friend.0) {
            // -- SNIP --
        }
    }

    Ok(())
}


fn counters(mut player: UniqueViewMut<Player>) {
    player.pellet_counter = player.pellet_counter.saturating_sub(1);
}

fn spawn(mut entities: EntitiesViewMut, mut vm_power_pellets: ViewMut<PowerPellet>) {
    let width = screen_width();
    let height = screen_height();

    let pellet_spawn_rate = if vm_power_pellets.is_empty() {
        POWER_PELLET_SPAWN_RATE / 2
    } else {
        POWER_PELLET_SPAWN_RATE
    };

    if rand::gen_range(0, pellet_spawn_rate) == 0 {
        let x = rand::gen_range(0.0, width - INIT_SIZE);
        let y = rand::gen_range(0.0, height - INIT_SIZE);

        entities.add_entity(
            &mut vm_power_pellets,
            PowerPellet(Square {
                x,
                y,
                size: INIT_SIZE * 2.0,
            }),
        );
    }
}
```

The syntax to add entities is very similar to adding components.\
But this time we need [`EntitiesViewMut`](https://docs.rs/shipyard/0.8/shipyard/struct.EntitiesViewMut.html).

With this change the `Player` is can now rest, stronger than ever.