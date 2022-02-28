use glam::{Vec2, Vec4};

pub fn pixel(uv: Vec2) -> Vec4 {
    Vec4::new(uv.x, uv.y, 0.25, 1.0)
}
