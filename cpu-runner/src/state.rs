use crate::{winit, Mutex};

pub struct State {
    pub window: winit::Window,
    pub primitives: Vec<raytracer::Primitive>,
    pub pixels: Mutex<pixels::Pixels>,
}
