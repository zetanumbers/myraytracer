use crate::{materials::Material, vision};
use std::ops;

pub struct Sphere<M: Material + ?Sized> {
    pub center: glam::Vec3,
    pub radius: f32,
    pub material: M,
}

impl<'a> Sphere<dyn Material + 'a> {
    fn hit_with_ray_impl(&self, ray: vision::Ray, t_r: ops::Range<f32>) -> Option<vision::Hit> {
        let oc = ray.origin - self.center;
        let a = ray.direction.length_squared();
        let b = oc.dot(ray.direction);
        let c = oc.length_squared() - self.radius.powi(2);
        let d = b.powi(2) - a * c;

        (d >= 0.).then(|| ())?;
        let d = d.sqrt();

        let t = Some((-b - d) / a)
            .filter(|t| t_r.contains(t))
            .or_else(|| Some((-b + d) / a))
            .filter(|t| t_r.contains(t))?;
        let at = ray.at(t);
        let normal = (at - self.center) / self.radius;

        Some(
            vision::Hit {
                t,
                at,
                normal,
                face: vision::Face::Front,
                material: &self.material,
            }
            .correct_face(ray),
        )
    }
}

impl<M: Material> vision::Visible for Sphere<M> {
    fn hit_with_ray(&self, ray: vision::Ray, t_r: ops::Range<f32>) -> Option<vision::Hit> {
        Sphere::<dyn Material>::hit_with_ray_impl(self, ray, t_r)
    }
}
