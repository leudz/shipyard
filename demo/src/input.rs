use crate::components::*;

use std::rc::{Rc};
use gloo_events::{EventListener};
use web_sys::{Event};
use shipyard::prelude::*;
use web_sys::{HtmlCanvasElement};

pub fn start(world:Rc<World>, canvas:&HtmlCanvasElement) {

    EventListener::new(canvas, "pointerdown", {
        let world = Rc::clone(&world);
        move |_e:&Event| {
            *world.borrow::<Unique<&mut Controller>>() = Controller::Adding; 
        }
    }).forget();

    EventListener::new(canvas, "pointerup", {
        let world = Rc::clone(&world);
        move |_e:&Event| {
            *world.borrow::<Unique<&mut Controller>>() = Controller::Waiting; 
        }
    }).forget();

}