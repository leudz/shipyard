# A lone square

Let's start with a blank window.

```rust,noplaypen
use macroquad::prelude::*;

#[macroquad::main("Square Eater")]
async fn main() {
    loop {
        next_frame().await
    }
}
```

Then let's add the player. This game is all about squares so naturally the player is one.

```rust,noplaypen
struct Square {
    x: f32,
    y: f32,
    size: f32,
}

struct Player {
    square: Square,
}
```

We can now it add the scene.

```rust,noplaypen
#[macroquad::main("Square Eater")]
async fn main() {
    let x = screen_width() / 2.0;
    let y = screen_height() / 2.0;
    let player = Player {
        square: Square { x, y, size: 15.0 },
    };

    loop {
        clear_background(WHITE);

        render(&player);

        next_frame().await
    }
}

impl Square {
    fn render(&self, color: Color) {
        draw_rectangle(self.x, self.y, self.size, self.size, color);
    }
}

fn render(player: &Player) {
    player.square.render(BLUE);
}
```

Our player looks a bit stiff, we can fix that.

```rust,noplaypen
async fn main() {
    // -- SNIP --
    let mut player = Player {
        square: Square { x, y, size: 15.0 },
    };

    loop {
        clear_background(WHITE);

        move_player(&mut player);
        render(&player);

        next_frame().await
    }
}

fn move_player(player: &mut Player) {
    let width = screen_width();
    let height = screen_height();
    let (x, y) = mouse_position();

    player.square.x = x.clamp(0.0, width - player.square.size);
    player.square.y = y.clamp(0.0, height - player.square.size);
}
```
