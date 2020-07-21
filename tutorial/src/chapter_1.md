# Humble Beginnings

Let's start by creating a new project and adding rltk to `Cargo.toml`:
```toml
rltk = "0.8.1"
```

Note that you can copy text in code blocks by clicking the icon at the top right corner of each block.

Then you can replace the content of `main.rs` by:
```rust, noplaypen
use rltk::{GameState, Rltk, RltkBuilder};

struct State;
impl GameState for State {
    fn tick(&mut self, ctx: &mut Rltk) {
        ctx.cls();
        ctx.print(1, 1, "Hello Rust World");
    }
}

fn main() -> rltk::BError {
    let context = RltkBuilder::simple80x50()
        .with_title("Roguelike Tutorial")
        .build()?;

    let gs = State;
    rltk::main_loop(context, gs)
}
```

This code will open a new console named "Roguelike Tutorial", composed of 80 * 50 characters.  
We're then initializing a `GameState` that every frame will clear the console and print "Hello Rust World" at the top left.

## A Wild ECS appears!

Let's add `shipyard` to our dependencies: 
```toml
shipyard = { git = "https://github.com/leudz/shipyard" }
```

We use github and not crates.io for now, we'll use 0.5 when it gets released.

An ECS will help us compose our game objects.  
A few years back object-oriented was the preferred pattern but the gamedev community has moved away from it. Rust also prefers composition over inheritance so we won't fight the language.  
In addition, ECS in Rust often makes ownership easier to deal with, we just delegate the burden to the ECS.

I've been using the abbreviation a few times already but ECS stands for Entity, Component, System.  
Components are data that you own, it can be a struct, an enum,...  
Entities are composed of multiple components and can be referred to using an [`EntityId`](https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html).  
Systems are actions on entities and components.

This is very theoretic so let's move to the practice!

## Adding our first entity

We'll need to import a few more things to `main.rs`:
```rust, noplaypen
use rltk::{GameState, Rltk, RltkBuilder, RGB};
use shipyard::{EntitiesViewMut, ViewMut, World};
```

Everything we'll do with the ECS will go through [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html), it holds all entities and components.

We'll also add these two structs:
```rust, noplaypen
struct Position {
    x: i32,
    y: i32,
}

struct Renderable {
    glyph: rltk::FontCharType,
    fg: RGB,
    bg: RGB,
}
```

`Renderable` defines how to render a character; `glyph` specify which character, `fg` the foreground color and `bg` the background color.

Let's add a [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html) to our `State`:
```rust, noplaypen
struct State {
    ecs: World
}
```
And replace its initialization in the `main` function:
```rust, noplaypen
let gs = State { ecs: World::new() };
```

Next we want to add our first entity with a couple of components:
```rust, noplaypen
fn initial_entities(
    mut entities: EntitiesViewMut,
    mut positions: ViewMut<Position>,
    mut renderables: ViewMut<Renderable>,
) {
    // we'll fill the body very soon
}
```

`initial_entities` takes three arguments, all of them are views. This is an important concept of shipyard.  
Every component in the [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html) is stored in a different storage and views let us access these storages.  
Here, we access the [`Entities`](https://docs.rs/shipyard/latest/shipyard/struct.Entities.html), `Position` and `Renderable` storages and we want to modify all of them.

The body of the function:
```rust, noplaypen
entities.add_entity(
    (&mut positions, &mut renderables),
    (
        Position { x: 40, y: 25 },
        Renderable {
            glyph: rltk::to_cp437('@'),
            fg: RGB::named(rltk::YELLOW),
            bg: RGB::named(rltk::BLACK),
        },
    ),
);
```

In this function we create a new entity with a `Position` and a `Renderable` component.

To add an entity we call [`Entities::add_entity`](https://docs.rs/shipyard/latest/shipyard/struct.Entities.html#method.add_entity). This method takes two arguments, the storages of the components we want to add and the components themselves.  
Since we want to add multiple components to multiple storages we use two tuples.  
[`add_entity`](https://docs.rs/shipyard/latest/shipyard/struct.Entities.html#method.add_entity) will give us back an [`EntityId`](https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html) that we can use to refer to this entity but we don't need it in this case.

A function with only views as arguments is a system. We'll [`run`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run) it right after creating `gs`:
```rust, noplaypen
gs.ecs.run(initial_entities);
```

## Rendering

We now have an entity with two components but haven't told `rltk` yet.  

- imports:
    ```diff
    - use shipyard::{EntitiesViewMut, ViewMut, World};
    ```
    ```rust, noplaypen
    use shipyard::{EntitiesViewMut, IntoIter, View, ViewMut, World};
    ```

- `State::tick`:
    ```diff
    - `ctx.print(1, 1, "Hello Rust World");`
    ```
    ```rust, noplaypen
    self.ecs
        .run(|positions: View<Position>, renderables: View<Renderable>| {
            for (pos, render) in (&positions, &renderables).iter() {
                ctx.set(pos.x, pos.y, render.fg, render.bg, render.glyph);
            }
        });
    ```

Here instead of a free function we use a closure but the same rule applies. As long as it only takes views as arguments: It's a system.  

We're also using [`iter`](https://docs.rs/shipyard/latest/shipyard/trait.IntoIter.html#tymethod.iter) from the [`IntoIter`](https://docs.rs/shipyard/latest/shipyard/trait.IntoIter.html) trait.  
It will give us all entities that have both a `Position` and `Renderable` components. In our case it'll be a single entity.  
We then call `Rltk::set` to render it.

## Final code

If you run this code you should see a yellow `@` in the middle of a fully black background.  

```rust, noplaypen
use rltk::{GameState, Rltk, RltkBuilder, RGB};
use shipyard::{EntitiesViewMut, IntoIter, View, ViewMut, World};

struct Position {
    x: i32,
    y: i32,
}
struct Renderable {
    glyph: rltk::FontCharType,
    fg: RGB,
    bg: RGB,
}

struct State {
    ecs: World,
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut Rltk) {
        ctx.cls();

        self.ecs
            .run(|positions: View<Position>, renderables: View<Renderable>| {
                for (pos, render) in (&positions, &renderables).iter() {
                    ctx.set(pos.x, pos.y, render.fg, render.bg, render.glyph);
                }
            });
    }
}

fn initial_entities(
    mut entities: EntitiesViewMut,
    mut positions: ViewMut<Position>,
    mut renderables: ViewMut<Renderable>,
) {
    entities.add_entity(
        (&mut positions, &mut renderables),
        (
            Position { x: 40, y: 25 },
            Renderable {
                glyph: rltk::to_cp437('@'),
                fg: RGB::named(rltk::YELLOW),
                bg: RGB::named(rltk::BLACK),
            },
        ),
    );
}

fn main() -> rltk::BError {
    let context = RltkBuilder::simple80x50()
        .with_title("Roguelike Tutorial")
        .build()?;
    let gs = State { ecs: World::new() };

    gs.ecs.run(initial_entities);

    rltk::main_loop(context, gs)
}
```

---

The "Shipyard Tutorial" is a derivative work of ["Roguelike Tutorial - In Rust"](https://bfnightly.bracketproductions.com/rustbook/) by [Herbert Wolverson](https://github.com/thebracket), used under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/legalcode). "Shipyard Tutorial" is licensed under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/legalcode) by Dylan Ancel.
