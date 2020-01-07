use shipyard::prelude::*;

fn taken_from_run() {
    static mut WORLD: Option<World> = None;

    unsafe { WORLD = Some(World::new::<(usize,)>()) };

    fn test() -> shipyard::internal::ViewMut<'static, usize> {
        let mut result = None;
        unsafe { WORLD.as_ref().unwrap() }.run::<&mut usize, _, _>(|usizes| result = Some(usizes));
        result.unwrap()
    }

    let _view = test();
    test();
}

fn main() {}
