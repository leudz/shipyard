# Spark

Let's infuse a bit of life into our friends.

```rust,noplaypen
use shipyard::{Component, IntoIter, Unique, UniqueView, UniqueViewMut, View, ViewMut, World};

const GROWTH_RATE: f32 = 0.15;
const MAX_SIZE: f32 = 25.0;

async fn main() {
    // -- SNIP --

        world.run(move_player);
        world.run(grow);
        world.run(render);

    // -- SNIP --
}


fn grow(mut vm_friend: ViewMut<Friend>) {
    for friend in (&mut vm_friend).iter() {
        let delta_size = (friend.0.size + GROWTH_RATE).min(MAX_SIZE) - friend.0.size;
        friend.0.size = friend.0.size + delta_size;
        friend.0.x = (friend.0.x - delta_size / 2.0).max(0.0);
        friend.0.y = (friend.0.y - delta_size / 2.0).max(0.0);
    }
}
```

`grow`'s code could be simpler but this version makes `Friends` grow from their center, which feels a lot more natural.

It appears our `Friends` want to come close to the `Player`, likely to give them a hug.

```rust,noplaypen
const SPEED: f32 = 1.5;

async fn main() {
    // -- SNIP --

        world.run(move_player);
        world.run(move_friends);
        world.run(grow);
        world.run(render);

    // -- SNIP --
}


impl Square {
    // -- SNIP --

    fn center(&self) -> Vec2 {
        vec2(self.x + self.size / 2.0, self.y + self.size / 2.0)
    }
}

fn move_friends(player: UniqueView<Player>, mut vm_friend: ViewMut<Friend>) {
    let mut dirs = vec![Vec2::ZERO; vm_friend.len()];

    for (friend, dir) in vm_friend.iter().zip(&mut dirs) {
        if friend.0.size <= player.square.size {
            continue;
        }

        let player_dir = player.square.center() - friend.0.center();

        *dir = player_dir.normalize();

        let mut neighbor_dir = Vec2::ZERO;

        for neighbor in vm_friend.iter() {
            if friend.0.center().distance_squared(neighbor.0.center())
                < friend.0.size * friend.0.size / 1.5
            {
                neighbor_dir +=
                    Vec2::new(friend.0.x - neighbor.0.x, friend.0.y - neighbor.0.y);
            }
        }

        *dir *= SPEED;

        *dir += neighbor_dir * 0.05;
    }

    let width = screen_width();
    let height = screen_height();
    for (friend, dir) in (&mut vm_friend).iter().zip(dirs) {
        if dir == Vec2::ZERO {
            continue;
        }

        friend.0.x = (friend.0.x + dir.x).clamp(0.0, width - friend.0.size);
        friend.0.y = (friend.0.y + dir.y).clamp(0.0, height - friend.0.size);
    }
}
```

As you can see, you can iterate views multiple times in the same system.\
We also prevent the `Friends` from overlapping by stirring them away from their neighbors.

But something doesn't feel right...

```rust,noplaypen
async fn main() {
    // -- SNIP --

        world.run(move_player);
        world.run(move_friends);
        world.run(grow);
        world.run(collision);
        world.run(render);

    // -- SNIP --
}

impl Square {
    // -- SNIP --

    fn collide(&self, other: &Square) -> bool {
        self.x + self.size >= other.x
            && self.x <= other.x + other.size
            && self.y + self.size >= other.y
            && self.y <= other.y + other.size
    }
}

fn collision(mut player: UniqueViewMut<Player>, v_friend: View<Friend>) {
    for friend in v_friend.iter() {
        if friend.0.size == MAX_SIZE && friend.0.collide(&player.square) {
            player.square.size -= 5.0 / 2.;

            if player.square.size < 5.0 {
                panic!("Murder");
            }
        }
    }
}
```

Oh my god! The "`Friends`" killed the `Player`!?