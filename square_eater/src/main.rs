use macroquad::prelude::*;
use shipyard::{
    AddComponent, AllStoragesViewMut, EntitiesViewMut, EntityId, Get, IntoIter, IntoWithId,
    SparseSet, UniqueView, UniqueViewMut, View, ViewMut, Workload, World,
};

const HEIGHT: i32 = 360;
const WIDTH: i32 = 640;
const INIT_SIZE: f32 = 5.;
const MAX_SIZE: f32 = 25.;
const GROWTH_RATE: f32 = 0.15;
const SPEED: f32 = 1.5;

struct Player {
    id: EntityId,
    is_invincible: bool,
    i_counter: u32,
    squagum: bool,
    squagum_counter: u32,
}

struct Squagum(Vec2);

struct Resistance(f32);

struct ToDelete;

#[derive(Debug)]
enum GameOver {
    Loose,
    Victory,
}

impl std::error::Error for GameOver {}

impl std::fmt::Display for GameOver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

fn new_square() -> Rect {
    Rect::new(
        rand::gen_range(MAX_SIZE / 2.0, WIDTH as f32 - MAX_SIZE / 2.),
        rand::gen_range(MAX_SIZE / 2.0, HEIGHT as f32 - MAX_SIZE / 2.),
        INIT_SIZE,
        INIT_SIZE,
    )
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Square Eater".to_owned(),
        window_width: WIDTH,
        window_height: HEIGHT,
        ..Default::default()
    }
}

fn init_world(world: &mut World) {
    let player = world.add_entity((Rect::new(0., 0., INIT_SIZE * 3., INIT_SIZE * 3.),));

    let _ = world.remove_unique::<Player>();

    world
        .add_unique(Player {
            id: player,
            is_invincible: false,
            i_counter: 0,
            squagum: false,
            squagum_counter: 0,
        })
        .unwrap();
    world.add_unique(Resistance(0.0)).unwrap();

    world.bulk_add_entity((0..7).map(|_| (new_square(), Resistance(0.0))));
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut world = World::new();

    init_world(&mut world);

    rand::srand(macroquad::miniquad::date::now() as u64);

    Workload::builder("")
        .with_system(&counters)
        .with_system(&move_player)
        .with_system(&move_square)
        .with_system(&grow_square)
        .with_system(&new_squares)
        .with_system(&collision)
        .with_system(&clean_up)
        .with_try_system(&game_over)
        .with_system(&render)
        .add_to_world(&world)
        .unwrap();

    let mut is_started = false;

    loop {
        if is_mouse_button_pressed(MouseButton::Left) {
            is_started = true;

            unsafe {
                get_internal_gl().quad_context.show_mouse(false);
            }
        }

        if is_started {
            clear_background(WHITE);

            match world
                .run_default()
                .map_err(shipyard::error::RunWorkload::custom_error)
            {
                Err(Some(err)) => {
                    match err.downcast_ref::<GameOver>().unwrap() {
                        GameOver::Loose => debug!("GameOver"),
                        GameOver::Victory => debug!("Victory"),
                    }

                    is_started = false;
                    world.clear();
                    init_world(&mut world);

                    continue;
                }
                _ => {}
            }
        } else {
            clear_background(BLACK);

            let text_dimensions = measure_text("Click to start", None, 40, 1.);
            draw_text(
                "Click to start",
                WIDTH as f32 / 2. - text_dimensions.0 / 2.,
                HEIGHT as f32 / 2. - text_dimensions.1 / 2.,
                40.,
                WHITE,
            );
        }

        next_frame().await
    }
}

fn counters(mut player: UniqueViewMut<Player>) {
    if player.is_invincible {
        player.i_counter += 1;

        if player.i_counter >= 10 {
            player.is_invincible = false;
            player.i_counter = 0;
        }
    }

    if player.squagum {
        player.squagum_counter += 1;

        if player.squagum_counter >= 120 {
            player.squagum = false;
            player.squagum_counter = 0;
        }
    }
}

fn move_player(player: UniqueView<Player>, mut rects: ViewMut<Rect>) {
    let mut player_rect = (&mut rects).get(player.id).unwrap();
    player_rect.move_to(mouse_position().into());

    player_rect.x = player_rect.x.max(player_rect.w / 2.);
    player_rect.x = player_rect.x.min(WIDTH as f32 - player_rect.w / 2.);
    player_rect.y = player_rect.y.max(player_rect.h / 2.);
    player_rect.y = player_rect.y.min(HEIGHT as f32 - player_rect.h / 2.);
}

fn move_square(
    player: UniqueView<Player>,
    mut rects: ViewMut<Rect>,
    mut resistances: ViewMut<Resistance>,
) {
    let player_rect = rects.get(player.id).unwrap().clone();

    for mut resistance in (&mut resistances).iter() {
        resistance.0 += 0.01;
    }

    let mut dirs = vec![Vec2::zero(); rects.len()];

    for ((id, rect), dir) in rects.iter().with_id().zip(&mut dirs) {
        if id != player.id && rect.w > player_rect.w && rect.h > player_rect.h {
            let player_dir = Vec2::new(
                player_rect.x - player_rect.w / 2.,
                player_rect.y - player_rect.h / 2.,
            ) - Vec2::new(rect.x - rect.w / 2., rect.y - rect.h / 2.);

            let mut neighbourg_dir = Vec2::zero();

            for (id, neighbourg) in rects.iter().with_id() {
                if id != player.id
                    && rect.point().distance_squared(neighbourg.point()) < rect.w * rect.h / 1.5
                {
                    neighbourg_dir += Vec2::new(rect.x - neighbourg.x, rect.y - neighbourg.y);
                }
            }

            if !player.squagum {
                *dir = player_dir.normalize();
            } else {
                *dir = -player_dir.normalize();
            }

            if rect.w == MAX_SIZE && rect.h == MAX_SIZE {
                *dir *= SPEED + resistances.get(id).unwrap().0;
            } else {
                *dir *= SPEED;
            }

            *dir += neighbourg_dir * 0.05;

            *dir += rect.point();

            if dir.x < INIT_SIZE / 2. {
                dir.x = INIT_SIZE / 2.;
            } else if dir.x > WIDTH as f32 - INIT_SIZE / 2. {
                dir.x = WIDTH as f32 - INIT_SIZE / 2.;
            }
            if dir.y < INIT_SIZE / 2. {
                dir.y = INIT_SIZE / 2.;
            } else if dir.y > HEIGHT as f32 - INIT_SIZE / 2. {
                dir.y = HEIGHT as f32 - INIT_SIZE / 2.;
            }
        }
    }

    for (mut rect, dir) in (&mut rects).iter().zip(dirs) {
        if dir != Vec2::zero() {
            rect.move_to(dir);
        }
    }
}

fn grow_square(player: UniqueView<Player>, mut rects: ViewMut<Rect>) {
    for (id, mut rect) in (&mut rects).iter().with_id() {
        if id != player.id {
            rect.h += GROWTH_RATE;
            rect.w += GROWTH_RATE;

            if rect.h > MAX_SIZE {
                rect.h = MAX_SIZE;
            }
            if rect.w > MAX_SIZE {
                rect.w = MAX_SIZE;
            }
        }
    }
}

fn new_squares(
    mut entities: EntitiesViewMut,
    mut rects: ViewMut<Rect>,
    mut resistances: ViewMut<Resistance>,
    mut squagums: ViewMut<Squagum>,
) {
    if rand::gen_range(0, 25) == 0 {
        entities.add_entity(
            (&mut rects, &mut resistances),
            (new_square(), Resistance(0.0)),
        );
    }
    if rand::gen_range(0, 150) == 0 {
        entities.add_entity(
            &mut squagums,
            Squagum(Vec2::new(
                rand::gen_range(0.0, WIDTH as f32),
                rand::gen_range(0.0, HEIGHT as f32),
            )),
        );
    }
}

fn collision(
    mut player: UniqueViewMut<Player>,
    mut rects: ViewMut<Rect>,
    squagums: View<Squagum>,
    mut to_delete: ViewMut<ToDelete>,
) {
    let mut player_rect = rects.get(player.id).unwrap().clone();

    for (id, squagum) in squagums.iter().with_id() {
        if player_rect.x - player_rect.w / 2. <= squagum.0.x
            && player_rect.x + player_rect.w / 2. >= squagum.0.x + INIT_SIZE
            && player_rect.y - player_rect.h / 2. <= squagum.0.y
            && player_rect.y + player_rect.h / 2. >= squagum.0.y + INIT_SIZE
        {
            player.squagum = true;
            to_delete.add_component_unchecked(id, ToDelete);
        }
    }

    let player_id = player.id;
    for (id, rect) in rects.iter().with_id().filter(|(id, _)| *id != player_id) {
        if rect.w == MAX_SIZE && rect.h == MAX_SIZE {
            if rect.x - rect.w / 2. <= player_rect.x + player_rect.w / 2.
                && rect.x + rect.w / 2. >= player_rect.x - player_rect.w / 2.
                && rect.y - rect.h / 2. <= player_rect.y + player_rect.h / 2.
                && rect.y + rect.h / 2. >= player_rect.y - player_rect.h / 2.
            {
                if player.squagum {
                    player_rect.w += INIT_SIZE / 4.;
                    player_rect.h += INIT_SIZE / 4.;
                    to_delete.add_component_unchecked(id, ToDelete);
                }

                if !player.is_invincible {
                    player.is_invincible = true;
                    player_rect.w -= INIT_SIZE / 2.;
                    player_rect.h -= INIT_SIZE / 2.;
                }
            }
        } else if player_rect.x >= rect.w
            && player_rect.h >= rect.h
            && player_rect.x - player_rect.w / 2. <= rect.x + rect.w / 2.
            && player_rect.x + player_rect.w / 2. >= rect.x - rect.w / 2.
            && player_rect.y - player_rect.h / 2. <= rect.y + rect.h / 2.
            && player_rect.y + player_rect.h / 2. >= rect.y - rect.h / 2.
        {
            player_rect.w += INIT_SIZE / 2.;
            player_rect.h += INIT_SIZE / 2.;
            to_delete.add_component_unchecked(id, ToDelete)
        }
    }

    if player_rect.w < INIT_SIZE || player_rect.h < INIT_SIZE {
        to_delete.add_component_unchecked(player.id, ToDelete);
    } else {
        if player_rect.w >= MAX_SIZE {
            player_rect.w = MAX_SIZE - 0.01;
        }
        if player_rect.h >= MAX_SIZE {
            player_rect.h = MAX_SIZE - 0.01;
        }

        *(&mut rects).get(player.id).unwrap() = player_rect;
    }
}

fn clean_up(mut all_storages: AllStoragesViewMut) {
    all_storages.delete_any::<SparseSet<ToDelete>>();
}

fn game_over(all_storages: AllStoragesViewMut) -> Result<(), GameOver> {
    let (player, rects) = all_storages
        .borrow::<(UniqueView<Player>, View<Rect>)>()
        .unwrap();

    if rects.contains(player.id) {
        if rects.len() == 1 {
            Err(GameOver::Victory)
        } else {
            Ok(())
        }
    } else {
        Err(GameOver::Loose)
    }
}

fn render(player: UniqueView<Player>, rects: View<Rect>, squagums: View<Squagum>) {
    let player_rect = rects.get(player.id).unwrap().clone();

    for (_, rect) in rects.iter().with_id().filter(|(id, _)| *id != player.id) {
        if rect.h == MAX_SIZE && rect.w == MAX_SIZE {
            draw_rectangle(
                rect.x - rect.w / 2.,
                rect.y - rect.h / 2.,
                rect.w,
                rect.h,
                RED,
            );
        } else if rect.w <= player_rect.w && rect.h <= player_rect.h {
            draw_rectangle(
                rect.x - rect.w / 2.,
                rect.y - rect.h / 2.,
                rect.w,
                rect.h,
                GREEN,
            );
        } else {
            draw_rectangle(
                rect.x - rect.w / 2.,
                rect.y - rect.h / 2.,
                rect.w,
                rect.h,
                GRAY,
            );
        }
    }

    for squagum in squagums.iter() {
        draw_rectangle(squagum.0.x, squagum.0.y, INIT_SIZE, INIT_SIZE, YELLOW);
    }

    draw_rectangle(
        player_rect.x - player_rect.w / 2.,
        player_rect.y - player_rect.h / 2.,
        player_rect.w,
        player_rect.h,
        if !player.squagum { BLUE } else { YELLOW },
    );
}
