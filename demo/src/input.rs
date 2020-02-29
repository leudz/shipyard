use crate::components::*;

use gloo_events::EventListener;
use shipyard::prelude::*;
use std::rc::Rc;
use web_sys::HtmlCanvasElement;

pub fn start(world: Rc<World>, canvas: &HtmlCanvasElement) {
    EventListener::new(canvas, "pointerdown", {
        let world = Rc::clone(&world);
        move |_| {
            *world.borrow::<Unique<&mut Controller>>() = Controller::Adding;
        }
    })
    .forget();

    EventListener::new(canvas, "pointerup", move |_| {
        *world.borrow::<Unique<&mut Controller>>() = Controller::Waiting;
    })
    .forget();
}
