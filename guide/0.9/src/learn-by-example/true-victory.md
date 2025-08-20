# True victory

It seems the `Friends` are able to copy the power pellets' spawning mechanic!
And they've learned to avoid the `Player` whenever they are powered up.

```rust,noplaypen
const FRIEND_SPAWN_RATE: u32 = 25;


fn move_friends(player: UniqueView<Player>, mut vm_friend: ViewMut<Friend>) {
        // -- SNIP --

        *dir = player_dir.normalize();

        if player.is_powered_up() {
            *dir = -*dir;
        }

        // -- SNIP --
}


fn spawn(
    mut entities: EntitiesViewMut,
    mut vm_friend: ViewMut<Friend>,
    mut vm_power_pellets: ViewMut<PowerPellet>,
) {
    // -- SNIP --

    if rand::gen_range(0, FRIEND_SPAWN_RATE) == 0 {
        let x = rand::gen_range(0.0, width - INIT_SIZE / 2.0);
        let y = rand::gen_range(0.0, height - INIT_SIZE / 2.0);

        entities.add_entity(
            &mut vm_friend,
            Friend(Square {
                x,
                y,
                size: INIT_SIZE,
            }),
        );
    }
}
```

Let's give the `Player` a little bit of help and a way to win again.
In many games, whenever the player is hit they'll turn invincible for a few frames.

```rust,noplaypen
async fn main() {
    // -- SNIP --

    let player = Player {
        // -- SNIP --
        i_counter: 0,
    };

    // -- SNIP --
}

struct Player {
    // -- SNIP --
    i_counter: u32,
}

impl Player {
    // -- SNIP --

    fn turn_invincible(&mut self) {
        self.i_counter = 5;
    }

    fn is_invincible(&self) -> bool {
        self.i_counter > 0
    }
}

fn collision(
    entities: EntitiesView,
    mut player: UniqueViewMut<Player>,
    v_friend: View<Friend>,
    v_power_pellets: View<PowerPellet>,
    mut vm_to_delete: ViewMut<ToDelete>,
) -> Result<(), GameOver> {
        // -- SNIP --

            if player.powered_up() {
                // -- SNIP --
            } else if player.is_invincible() {
                continue;
            }

            player.square.size -= INIT_SIZE / 2.;
            player.turn_invincible();

            // -- SNIP --
}

fn counters(mut player: UniqueViewMut<Player>) {
    player.pellet_counter = player.pellet_counter.saturating_sub(1);
    player.i_counter = player.i_counter.saturating_sub(1);
}
```

We'll conclude this guide by allowing the `Player` to win.

```rust,noplaypen
use shipyard::{
    AllStoragesViewMut, Component, EntitiesView, EntitiesViewMut, IntoIter, IntoWithId,
    IntoWorkload, IntoWorkloadTrySystem, SparseSet, Unique, UniqueView, UniqueViewMut, View,
    ViewMut, Workload, World,
};

async fn main() {
    // -- SNIP --

    for _ in 0..5 {
        let _entity_id = world.add_entity(Friend::new());
    }

    world.add_workload(main_loop);

    loop {
        clear_background(WHITE);

        world.run_workload(main_loop);

        next_frame().await
    }
}

#[derive(Debug)]
enum GameOver {
    Defeat,
    Victory,
}

impl std::fmt::Display for GameOver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}
impl std::error::Error for GameOver {}

fn main_loop() -> Workload {
    (
        move_player,
        move_friends,
        grow,
        counters,
        spawn,
        collision,
        clean_up,
        check_game_over.into_workload_try_system().unwrap(),
        render,
    )
        .into_workload()
}


fn collision(
    // -- SNIP --
) {
    // -- SNIP --

            player.square.size -= INIT_SIZE / 2.;
            player.turn_invincible();

            // No more return
        } else if player.square.size >= friend.0.size && player.square.collide(&friend.0) {
            // -- SNIP --
        }
    }

    // No more Ok(())
}

fn check_game_over(player: UniqueView<Player>, v_friends: View<Friend>) -> Result<(), GameOver> {
    if player.square.size < INIT_SIZE {
        Err(GameOver::Defeat)
    } else if v_friends.is_empty() {
        Err(GameOver::Victory)
    } else {
        Ok(())
    }
}
```

[`Workload`](https://docs.rs/shipyard/0.9/shipyard/struct.Workload.html)s are a collection of systems.\
We only have a single [`Workload`](https://docs.rs/shipyard/0.9/shipyard/struct.Workload.html) in our game since it's quite small.\
You would usually have smaller [`Workload`](https://docs.rs/shipyard/0.9/shipyard/struct.Workload.html)s that make up larger ones.\
Apart from organization, [`Workload`](https://docs.rs/shipyard/0.9/shipyard/struct.Workload.html)s are automatically run across multiple threads, which can usually boost performance.

The last touch is to handle `check_game_over`'s return value.\
We use [`into_workload_try_system`](https://docs.rs/shipyard/0.9/shipyard/trait.IntoWorkloadTrySystem.html#tymethod.into_workload_try_system) to explicitly inform the [`Workload`](https://docs.rs/shipyard/0.9/shipyard/struct.Workload.html) that this system might return something, but we don't handle it anywhere.

```rust,noplaypen
async fn main() {
    // -- SNIP --

    loop {
        clear_background(WHITE);

        if let Err(Some(game_over)) = world
            .run_workload(main_loop)
            .map_err(shipyard::error::RunWorkload::custom_error)
        {
            match game_over.downcast_ref::<GameOver>().unwrap() {
                GameOver::Defeat => panic!("Murder"),
                GameOver::Victory => panic!("Victory!"),
            }
        }

        next_frame().await
    }
}
```

After some type juggling, we can get our result back.

## Conclusion

This concludes the example guide.\
You've encountered the main ways you can interact with entities, components and systems.\
The following reference guide delves deeper into details and is a good place to come back to once you start your own project.

---

You may be wondering where are the floors, the shop,...\
Your mission, should you choose to accept it is to build the rest of the game.

Each new floor reached, the `Friends` gain one of these bonuses:
- start size +0.5
- growth rate +0.05
- speed +0.1
- number +3
- spawn rate +4

Each floor, new or not, the `Player` chooses between:
- start size +3.0 (capped at 3)
- power up duration +10 (capped at 10)
- power up spawn rate +10 (capped at 10)
- size on eat +0.5 (capped at 10)
- defense +0.4 (capped at 5)

The game alternates between floor and shop.\
Each floor a total of `(floor_number + 1) * 2` `Friends` spawn.\
If the `Player` is able to eat all `Friends`, they move to the next floor.\
If not, they stay on the same floor but with a visit to the shop.