mod components;
mod map;
mod player;

use components::{Position, Renderable};
use map::Map;
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

        self.ecs.run_with_data(Map::draw, ctx);
        self.ecs
            .run(|positions: View<Position>, renderables: View<Renderable>| {
                for (pos, render) in (&positions, &renderables).iter() {
                    ctx.set(pos.x, pos.y, render.fg, render.bg, render.glyph);
                }
            });
    }
}

fn initial_entities(
    [x, y]: [i32; 2],
    mut entities: EntitiesViewMut,
    mut positions: ViewMut<Position>,
    mut renderables: ViewMut<Renderable>,
) {
    entities.add_entity(
        (&mut positions, &mut renderables),
        (
            Position { x, y },
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

    let map = Map::create_dungeon(80, 50);
    gs.ecs
        .run_with_data(initial_entities, map.rooms[0].center());
    gs.ecs.add_unique(map);

    rltk::main_loop(context, gs)
}
