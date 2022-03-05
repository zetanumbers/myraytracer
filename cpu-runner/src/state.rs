use crate::{winit, Mutex};

pub struct State {
    pub window: winit::Window,
    pub primitives: Vec<Box<dyn raytracer::Visible + Send + Sync>>,
    pub pixels: Mutex<pixels::Pixels>,
}

impl State {
    pub fn primitive_refs(&self) -> Vec<&(dyn raytracer::Visible + Send + Sync)> {
        self.primitives.iter().map(|b| &**b).collect::<Vec<_>>()
    }
}
