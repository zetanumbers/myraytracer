use crate::{winit, Mutex};

pub struct State {
    pub window: winit::Window,
    pub world: raytracer::World,
    pub pixels: Mutex<pixels::Pixels>,
}
