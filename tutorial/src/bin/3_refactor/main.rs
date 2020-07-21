mod components;
mod map;
mod player;

use components::{Position, Renderable};
use map::{create_map, draw_map};
use player::move_player;
use rltk::{GameState, Rltk, RGB};
use shipyard::{EntitiesViewMut, IntoIter, View, ViewMut, World};

struct State {
    ecs: World,
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut Rltk) {
        ctx.cls();

        self.ecs.run_with_data(move_player, ctx);

        self.ecs.run_with_data(draw_map, ctx);
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
    use rltk::RltkBuilder;

    let context = RltkBuilder::simple80x50()
        .with_title("Roguelike Tutorial")
        .build()?;
    let gs = State { ecs: World::new() };

    gs.ecs.run(initial_entities);
    gs.ecs.add_unique(create_map());

    rltk::main_loop(context, gs)
}
