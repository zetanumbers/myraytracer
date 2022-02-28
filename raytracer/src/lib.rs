use glam::{vec3, vec4, Vec3, Vec4};

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
        self.origin + self.direction * t
    }
}

pub struct World {
    spheres: Vec<Sphere>,
}

impl Default for World {
    fn default() -> Self {
        World {
            spheres: vec![Sphere {
                center: vec3(0., 0., -1.),
                radius: 0.5,
            }],
        }
    }
}

impl World {
    pub fn color(&self, ray: Ray) -> Vec4 {
        if let Some(r) = ray.hit(self) {
            return Vec4::from((0.5 * (r.normal + Vec3::ONE), 1.));
        }

        let t = 0.5 * (ray.direction.normalize_or_zero().y + 1.);
        Vec4::ONE.lerp(vec4(0.25, 0.49, 1.0, 1.0), t)
    }
}

trait Hit {
    fn hit_with_ray(&self, ray: Ray) -> Option<HitReport>;
}

struct HitReport {
    t: f32,
    at: Vec3,
    normal: Vec3,
}

impl Ray {
    fn hit(self, visible: &impl Hit) -> Option<HitReport> {
        visible.hit_with_ray(self)
    }
}

impl Hit for World {
    fn hit_with_ray(&self, ray: Ray) -> Option<HitReport> {
        self.spheres.iter().find_map(|s| ray.hit(s))
    }
}

#[derive(Clone, Copy)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

impl Hit for Sphere {
    fn hit_with_ray(&self, ray: Ray) -> Option<HitReport> {
        let oc = ray.origin - self.center;
        let a = ray.direction.length_squared();
        let b = oc.dot(ray.direction);
        let c = oc.length_squared() - self.radius.powi(2);
        let d = b.powi(2) - a * c;

        let t = (d >= 0.).then(|| (-b - d.sqrt()) / a)?;
        if t < 0.0 {
            return None;
        }
        let at = ray.at(t);
        let normal = (at - self.center) / self.radius;

        Some(HitReport { t, at, normal })
    }
}
