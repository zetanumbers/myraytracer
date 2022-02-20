#![feature(slice_as_chunks)]

mod ray;

use std::time;

use glam::{vec3, Vec3, Vec4};
use raytracer_common::{render_fn, RawRenderer, Renderer};

use crate::ray::Ray;

fn hit_sphere(center: Vec3, radius: f32, ray: Ray) -> bool {
    let oc = ray.origin - center;
    let a = ray.direction.length_squared();
    let b = 2.0 * oc.dot(ray.direction);
    let c = oc.length_squared() - radius.powi(2);
    let d = b.powi(2) - 4.0 * a * c;
    d > 0.0
}

fn ray_color(ray: Ray) -> Vec3 {
    if hit_sphere(vec3(0.0, 0.0, -1.0), 0.5, ray) {
        return vec3(1.0, 0.0, 0.0);
    }
    let n = ray.direction.normalize();
    let t = 0.5 * (n.y + 1.0);
    vec3(1.0, 1.0, 1.0).lerp(vec3(0.5, 0.7, 1.0), t)
}

#[no_mangle]
pub unsafe extern "C" fn new(width: usize, height: usize) -> RawRenderer {
    let aspect_ratio = width as f32 / height as f32;

    let viewport_height = 2.0;
    let viewport_width = viewport_height * aspect_ratio;
    const FOCAL_LENGTH: f32 = 1.0;

    const ORIGIN: Vec3 = Vec3::ZERO;
    let horizontal = Vec3::X * viewport_width;
    let vertical = Vec3::Y * viewport_height;
    let top_left_corner = ORIGIN - horizontal / 2.0 - vertical / 2.0 + Vec3::Z * FOCAL_LENGTH;

    let mut pixel_indices = 0..width * height;
    Renderer::from(Box::pin(render_fn(move |frame: &mut [u8]| {
        if pixel_indices.is_empty() {
            return false;
        }

        let (frame, rest) = frame.as_chunks_mut::<4>();
        assert_eq!(frame.len(), width * height);
        assert_eq!(rest, []);

        let frame_duration = time::Duration::from_secs_f64(1.0 / 60.0);

        let mut batch_start = time::Instant::now();
        let finish = batch_start + frame_duration;

        let mut approx_pixels = 1;
        while approx_pixels != 0 {
            for i in (&mut pixel_indices).take(approx_pixels) {
                let u = (i % width) as f32 / (width - 1) as f32;
                let v = (height - 1 - i / width) as f32 / (height - 1) as f32;

                let r = Ray::new(
                    ORIGIN,
                    top_left_corner + horizontal * u + vertical * v - ORIGIN,
                );

                let color = ray_color(r);

                frame[i] = [
                    (color.x * 255.0) as u8,
                    (color.y * 255.0) as u8,
                    (color.z * 255.0) as u8,
                    255,
                ];
            }

            let batch_finish = time::Instant::now();
            let elapsed = batch_finish - batch_start;
            batch_start = batch_finish;

            approx_pixels = if elapsed.is_zero() {
                approx_pixels * 2
            } else {
                ((finish - batch_finish).as_secs_f64() / elapsed.as_secs_f64())
                    .min(pixel_indices.len() as _)
                    .floor() as _
            };
        }

        true
    })))
    .into_raw()
}
