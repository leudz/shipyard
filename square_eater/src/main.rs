use macroquad::prelude::*;
use shipyard::{
    AddComponent, AllStoragesViewMut, Component, EntitiesViewMut, IntoIter, IntoWithId,
    IntoWorkload, IntoWorkloadTrySystem, SparseSet, Unique, UniqueView, UniqueViewMut, View,
    ViewMut, Workload, World,
};

const WIDTH: i32 = 640;
const HEIGHT: i32 = 360;
const INIT_SIZE: f32 = 5.;
const MAX_SIZE: f32 = 25.;
const GROWTH_RATE: f32 = 0.15;
const SPEED: f32 = 1.5;
const ACCELERATION_RATE: f32 = 0.01;
const SQUARE_SPAWN_RATE: u32 = 25;
const SQUAGUM_SPAWN_RATE: u32 = 150;

#[derive(Component)]
struct MyRect(macroquad::prelude::Rect);

#[derive(Unique)]
struct Player {
    is_invincible: bool,
    i_counter: u32,
    squagum: bool,
    squagum_counter: u32,
    rect: Rect,
}

#[derive(Component)]
struct Squagum(Vec2);
#[derive(Component)]
struct Acceleration(f32);
#[derive(Component)]
struct ToDelete;

#[derive(Debug, Component)]
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

/// generates a new random square.
fn new_square() -> (MyRect, Acceleration) {
    (
        MyRect(Rect {
            x: rand::gen_range(MAX_SIZE / 2.0, WIDTH as f32 - MAX_SIZE / 2.),
            y: rand::gen_range(MAX_SIZE / 2.0, HEIGHT as f32 - MAX_SIZE / 2.),
            w: INIT_SIZE,
            h: INIT_SIZE,
        }),
        Acceleration(0.),
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
    let _ = world.remove_unique::<Player>();

    world.add_unique(Player {
        is_invincible: false,
        i_counter: 0,
        squagum: false,
        squagum_counter: 0,
        rect: Rect::new(0., 0., INIT_SIZE * 3., INIT_SIZE * 3.),
    });

    world.bulk_add_entity((0..7).map(|_| new_square()));
}

fn main_loop() -> Workload {
    (
        counters,
        move_player,
        move_square,
        grow_square,
        new_squares,
        collision,
        clean_up.into_workload_try_system().unwrap(),
        render,
    )
        .into_workload()
}

// Entry point of the program
#[macroquad::main(window_conf)]
async fn main() {
    let mut world = World::new();

    init_world(&mut world);

    // seed the random number generator with a random value
    rand::srand(macroquad::miniquad::date::now() as u64);

    world.add_workload(main_loop);

    let mut is_started = false;
    loop {
        if is_started {
            clear_background(WHITE);

            if let Err(Some(err)) = world
                .run_default_workload()
                .map_err(shipyard::error::RunWorkload::custom_error)
            {
                match err.downcast_ref::<GameOver>().unwrap() {
                    GameOver::Loose => debug!("GameOver"),
                    GameOver::Victory => debug!("Victory"),
                }

                is_started = false;
                world.clear();
                init_world(&mut world);
            }
        } else {
            if is_mouse_button_pressed(MouseButton::Left) {
                is_started = true;

                unsafe {
                    get_internal_gl().quad_context.show_mouse(false);
                }
            }

            clear_background(BLACK);

            let text_dimensions = measure_text("Click to start", None, 40, 1.);
            draw_text(
                "Click to start",
                WIDTH as f32 / 2. - text_dimensions.width / 2.,
                HEIGHT as f32 / 2. - text_dimensions.height / 2.,
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

fn move_player(mut player: UniqueViewMut<Player>) {
    let (x, y) = mouse_position();
    player.rect.x = x.clamp(player.rect.w / 2., WIDTH as f32 - player.rect.w / 2.);
    player.rect.y = y.clamp(player.rect.h / 2., HEIGHT as f32 - player.rect.h / 2.);
}

fn move_square(
    player: UniqueView<Player>,
    mut rects: ViewMut<MyRect>,
    mut accelerations: ViewMut<Acceleration>,
) {
    for acceleration in (&mut accelerations).iter() {
        acceleration.0 += ACCELERATION_RATE;
    }

    let mut dirs = vec![Vec2::ZERO; rects.len()];

    for ((id, MyRect(rect)), dir) in rects.iter().with_id().zip(&mut dirs) {
        if rect.w > player.rect.w && rect.h > player.rect.h {
            let player_dir = player.rect.point()
                - Vec2::new(player.rect.w / 2., player.rect.h / 2.)
                - Vec2::new(rect.x - rect.w / 2., rect.y - rect.h / 2.);

            *dir = player_dir.normalize();

            if player.squagum {
                *dir = -*dir;
            }

            let mut neighbourg_dir = Vec2::ZERO;

            for MyRect(neighbourg) in rects.iter() {
                if rect.point().distance_squared(neighbourg.point()) < rect.w * rect.h / 1.5 {
                    neighbourg_dir += Vec2::new(rect.x - neighbourg.x, rect.y - neighbourg.y);
                }
            }

            if rect.w == MAX_SIZE && rect.h == MAX_SIZE {
                *dir *= SPEED + accelerations[id].0;
            } else {
                *dir *= SPEED;
            }

            *dir += rect.point() + neighbourg_dir * 0.05;

            dir.x = dir.x.clamp(INIT_SIZE / 2., WIDTH as f32 - INIT_SIZE / 2.);
            dir.y = dir.y.clamp(INIT_SIZE / 2., HEIGHT as f32 - INIT_SIZE / 2.);
        }
    }

    for (rect, dir) in (&mut rects).iter().zip(dirs) {
        if dir != Vec2::ZERO {
            rect.0.move_to(dir);
        }
    }
}

fn grow_square(mut rects: ViewMut<MyRect>) {
    for rect in (&mut rects).iter() {
        rect.0.w = (rect.0.w + GROWTH_RATE).min(MAX_SIZE);
        rect.0.h = (rect.0.h + GROWTH_RATE).min(MAX_SIZE);
    }
}

fn new_squares(
    mut entities: EntitiesViewMut,
    mut rects: ViewMut<MyRect>,
    mut accelerations: ViewMut<Acceleration>,
    mut squagums: ViewMut<Squagum>,
) {
    if rand::gen_range(0, SQUARE_SPAWN_RATE) == 0 {
        entities.add_entity((&mut rects, &mut accelerations), new_square());
    }

    if rand::gen_range(0, SQUAGUM_SPAWN_RATE) == 0 {
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
    rects: View<MyRect>,
    squagums: View<Squagum>,
    mut to_delete: ViewMut<ToDelete>,
) {
    for (id, squagum) in squagums.iter().with_id() {
        if player.rect.contains(squagum.0)
            || player
                .rect
                .contains(squagum.0 + Vec2::new(INIT_SIZE, INIT_SIZE))
        {
            player.squagum = true;
            to_delete.add_component_unchecked(id, ToDelete);
        }
    }

    for (id, rect) in rects.iter().with_id() {
        if rect.0.w == MAX_SIZE
            && rect.0.h == MAX_SIZE
            && rect.0.x - rect.0.w / 2. <= player.rect.x + player.rect.w / 2.
            && rect.0.x + rect.0.w / 2. >= player.rect.x - player.rect.w / 2.
            && rect.0.y - rect.0.h / 2. <= player.rect.y + player.rect.h / 2.
            && rect.0.y + rect.0.h / 2. >= player.rect.y - player.rect.h / 2.
        {
            if player.squagum {
                player.rect.w = (player.rect.w + INIT_SIZE / 4.).min(MAX_SIZE - 0.01);
                player.rect.h = (player.rect.h + INIT_SIZE / 4.).min(MAX_SIZE - 0.01);
                to_delete.add_component_unchecked(id, ToDelete);
            }

            if !player.is_invincible {
                player.is_invincible = true;
                player.rect.w -= INIT_SIZE / 2.;
                player.rect.h -= INIT_SIZE / 2.;
            }
        } else if player.rect.x >= rect.0.w
            && player.rect.h >= rect.0.h
            && player.rect.x - player.rect.w / 2. <= rect.0.x + rect.0.w / 2.
            && player.rect.x + player.rect.w / 2. >= rect.0.x - rect.0.w / 2.
            && player.rect.y - player.rect.h / 2. <= rect.0.y + rect.0.h / 2.
            && player.rect.y + player.rect.h / 2. >= rect.0.y - rect.0.h / 2.
        {
            player.rect.w = (player.rect.w + INIT_SIZE / 2.).min(MAX_SIZE - 0.01);
            player.rect.h = (player.rect.h + INIT_SIZE / 2.).min(MAX_SIZE - 0.01);
            to_delete.add_component_unchecked(id, ToDelete)
        }
    }
}

fn clean_up(mut all_storages: AllStoragesViewMut) -> Result<(), GameOver> {
    all_storages.delete_any::<SparseSet<ToDelete>>();

    let (player, rects) = all_storages
        .borrow::<(UniqueView<Player>, View<MyRect>)>()
        .unwrap();

    if player.rect.w < INIT_SIZE || player.rect.h < INIT_SIZE {
        Err(GameOver::Loose)
    } else {
        if rects.is_empty() {
            Err(GameOver::Victory)
        } else {
            Ok(())
        }
    }
}

fn render(player: UniqueView<Player>, rects: View<MyRect>, squagums: View<Squagum>) {
    for MyRect(rect) in rects.iter() {
        draw_rectangle(
            rect.x - rect.w / 2.,
            rect.y - rect.h / 2.,
            rect.w,
            rect.h,
            if rect.h == MAX_SIZE && rect.w == MAX_SIZE {
                RED
            } else if rect.w > player.rect.w && rect.h > player.rect.h {
                GRAY
            } else {
                GREEN
            },
        );
    }

    for squagum in squagums.iter() {
        draw_rectangle(squagum.0.x, squagum.0.y, INIT_SIZE, INIT_SIZE, YELLOW);
    }

    draw_rectangle(
        player.rect.x - player.rect.w / 2.,
        player.rect.y - player.rect.h / 2.,
        player.rect.w,
        player.rect.h,
        if !player.squagum { BLUE } else { YELLOW },
    );
}
