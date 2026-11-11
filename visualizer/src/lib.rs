//! UI-independent parts of the Shipyard visualizer.
//!
//! These modules don't depend on a renderer, so their tests run with `--no-default-features`.

pub mod camera;
pub mod facts;
pub mod highlight;
pub mod import;
pub mod inspectors;
pub mod layout;
pub mod model;
pub mod names;
pub mod overview;
pub mod panels;
pub mod search;
pub mod state;
