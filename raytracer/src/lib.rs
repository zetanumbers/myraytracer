use glam::{vec4, Vec3, Vec4};

#[derive(Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Ray { origin, direction }
    }

    pub fn at(self, t: f32) -> Vec3 {
        self.origin * t + self.direction
    }
}

pub struct World {}

impl World {
    pub fn color(&self, ray: Ray) -> Vec4 {
        let t = 0.5 * (ray.direction.normalize_or_zero().y + 1.);
        Vec4::ONE.lerp(vec4(0.25, 0.49, 1.0, 1.0), t)
    }
}
