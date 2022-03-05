use crate::primitives::{Hit, Visible};
use std::ops;

#[derive(Clone, Copy)]
pub struct Ray {
    pub origin: glam::Vec3,
    pub direction: glam::Vec3,
}

impl Ray {
    pub fn new(origin: glam::Vec3, direction: glam::Vec3) -> Self {
        Ray { origin, direction }
    }

    pub fn at(self, t: f32) -> glam::Vec3 {
        self.origin + self.direction * t
    }

    pub fn hit<P>(self, visible: &P, t_r: ops::Range<f32>) -> Option<Hit>
    where
        P: Visible + ?Sized,
    {
        visible.hit_with_ray(self, t_r)
    }

    pub fn hit_collection<'a, I, T>(self, iter: I, mut t_r: ops::Range<f32>) -> Option<Hit<'a>>
    where
        I: IntoIterator<Item = &'a T>,
        T: Visible + ?Sized + 'a,
    {
        let mut out = None;
        for v in iter {
            if let Some(hit) = self.hit(v, t_r.clone()) {
                out = Some(hit);
                t_r.end = hit.t;
            }
        }
        out
    }
}
