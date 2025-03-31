# Breather

Let's refactor a little to give us time to process this betrayal and think of a way to get our revenge.\
You can move things around and maybe create modules.

We're using an initial size in a few spots, we can abstract it.

```rust,noplaypen
const INIT_SIZE: f32 = 5.0;

async fn main() {
    // -- SNIP --

    let player = Player {
        square: Square {
            x,
            y,
            size: INIT_SIZE * 3.0,
        },
    };

    // -- SNIP --
}

impl Friend {
    fn new() -> Friend {
        // -- SNIP --

        Friend(Square {
            x: gen_range(0.0, width - INIT_SIZE),
            y: gen_range(0.0, height - INIT_SIZE),
            size: INIT_SIZE,
        })
    }
}

fn collision(mut player: UniqueViewMut<Player>, v_friend: View<Friend>) {
    for friend in v_friend.iter() {
        if friend.0.size == MAX_SIZE && friend.0.collide(&player.square) {
            player.square.size -= INIT_SIZE / 2.;

            if player.square.size < INIT_SIZE {
                panic!("Murder");
            }
        }
    }
}
```

We can also handle the game over a little cleaner.

```rust,noplaypen
enum GameOver {
    Defeat,
}

async fn main() {
    // -- SNIP --

    loop {
        // -- SNIP --

        if world.run(collision).is_err() {
            panic!("Murder");
        }

        // -- SNIP --
    }
}

fn collision(mut player: UniqueViewMut<Player>, v_friend: View<Friend>) -> Result<(), GameOver> {
    for friend in v_friend.iter() {
        if friend.0.size == MAX_SIZE && friend.0.collide(&player.square) {
            player.square.size -= INIT_SIZE / 2.;

            if player.square.size < INIT_SIZE {
                return Err(GameOver::Defeat);
            }
        }
    }

    Ok(())
}
```

Systems can return any type, [`World::run`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html#method.run) then returns when the function returns.\
Moving the panic to `main` isn't a big change but it allows a better control of what happens which will be useful later on.

To conclude this chapter we can better show the duplicity of the "`Friends`".

```rust,noplaypen
fn render(player: UniqueView<Player>, v_friend: View<Friend>) {
    for friend in v_friend.iter() {
        if friend.0.size == MAX_SIZE {
            friend.0.render(RED);
        } else if friend.0.size > player.square.size {
            friend.0.render(GRAY);
        } else {
            friend.0.render(GREEN);
        }
    }

    player.square.render(BLUE);
}
```