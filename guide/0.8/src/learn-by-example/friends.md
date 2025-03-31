# Friends

Our player has the whole window to explore but they're feeling lonely.\
We can add a few friends.

```rust,noplaypen
struct Friend(Square);
```

We could store them in a `Vec<Friend>` but you're not here for a macroquad tutorial.\
Instead we'll store them in a [`World`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html).

[`World`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html) is shipyard's main type.\
It's where everything is stored, from components to entities to systems.

In this guide, I'll be explicit about `shipyard` imports but you could `use shipyard::*;` if you prefer.

```rust,noplaypen
use macroquad::rand::gen_range;
use shipyard::{Component, World};

#[macroquad::main("Square Eater")]
async fn main() {
    rand::srand(macroquad::miniquad::date::now() as u64);

    // -- SNIP --

    let mut world = World::new();

    for _ in 0..5 {
        let _entity_id = world.add_entity(Friend::new());
    }

    loop {
        clear_background(WHITE);

        move_player(&mut player);
        render(&player, &world);

        next_frame().await
    }
}

fn render(player: &Player, world: &World) {
    for friend in &mut world.iter::<&Friend>() {
        friend.0.render(GREEN);
    }

    player.square.render(BLUE);
}

impl Friend {
    fn new() -> Friend {
        let width = screen_width();
        let height = screen_height();

        Friend(Square {
            x: gen_range(0.0, width - 5.0),
            y: gen_range(0.0, height - 5.0),
            size: 5.0,
        })
    }
}
```

This won't compile just yet, as `Friend` is not a [`Component`](https://docs.rs/shipyard/0.8/shipyard/trait.Component.html).\
Some ECS require you to explicitly specify which types are components and some don't.\
One of the reasons shipyard requires it is to easily identify components in codebases.\
With small projects, this isn't a big issue but as the number of lines grow, you'll have to find a way to identify components. This could be moving types to a `component.rs`, but I'd rather have modules split based on what they do.

Let's add the missing piece.

```rust,noplaypen
#[derive(Component)]
struct Friend(Square);
```

Now [`add_entity`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html#method.add_entity) can create 5 entities that are each composed of a single component.\
Every entity is identified with an [`EntityId`](https://docs.rs/shipyard/0.8/shipyard/struct.EntityId). It's a small handle that you can copy.\
And [`iter`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html#method.iter) let us iterate over components.

We can move `Player` into the [`World`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html) to simplify our code a little.\
We only have a single `Player` and it will only ever have a single component.\
For this kind of entities, shipyard has [`Unique`](https://docs.rs/shipyard/0.8/shipyard/trait.Unique.html) components.

```rust,noplaypen
use shipyard::{Component, Unique, World};

async fn main() {
    // -- SNIP --
    let mut world = World::new();
    let player = Player {
        square: Square { x, y, size: 15.0 },
    };

    world.add_unique(player);
    // -- SNIP --

    loop {
        clear_background(WHITE);

        move_player(&world);
        render(&world);

        next_frame().await
    }
}

#[derive(Unique)]
struct Player {
    square: Square,
}

fn render(world: &World) {
    for friend in &mut world.iter::<&Friend>() {
        friend.0.render(GREEN);
    }

    let player = world.get_unique::<&Player>().unwrap();

    player.square.render(BLUE);
}

fn move_player(world: &World) {
    let width = screen_width();
    let height = screen_height();
    let (x, y) = mouse_position();
    let mut player = world.get_unique::<&mut Player>().unwrap();

    player.square.x = x.clamp(0.0, width - player.square.size);
    player.square.y = y.clamp(0.0, height - player.square.size);
}
```

We can simply further by using views. Views are temporary access of components.

```rust,noplaypen
use shipyard::{Component, IntoIter, Unique, UniqueView, UniqueViewMut, View, World};

async fn main() {
    // -- SNIP --

    loop {
        clear_background(WHITE);

        world.run(move_player);
        world.run(render);

        next_frame().await
    }
}

fn render(player: UniqueView<Player>, v_friend: View<Friend>) {
    for friend in v_friend.iter() {
        friend.0.render(GREEN);
    }

    player.square.render(BLUE);
}

fn move_player(mut player: UniqueViewMut<Player>) {
    let width = screen_width();
    let height = screen_height();
    let (x, y) = mouse_position();

    player.square.x = x.clamp(0.0, width - player.square.size);
    player.square.y = y.clamp(0.0, height - player.square.size);
}
```

You've just written your first systems.\
With shipyard, all functions that have only views as arguments are systems.\
The [`World`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html) understands these functions and provides the desired components automatically.

The `v_`/`vm_` prefix for views is a convention that some `shipyard` users use. I'll follow it throughout the guide.