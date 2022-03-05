use crate::{winit, State};
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use std::{num::NonZeroUsize, sync::Arc, thread, time};

pub struct Handle {
    thread: Option<thread::JoinHandle<()>>,
    continue_: Arc<()>,
}

impl Handle {
    pub fn new(state: Arc<State>) -> Self {
        let continue_ = Arc::new(());

        let thread = Some(thread::spawn({
            let continue_ = Arc::downgrade(&continue_);
            move || {
                let start = time::Instant::now();

                let input = raytracer::Input {
                    primitives: &state.primitives,
                };

                let size = state.window.inner_size();
                let size = winit::PhysicalSize {
                    width: size.width as usize,
                    height: size.height as usize,
                };

                log::info!(
                    "Starting a renderer thread for window size {}x{}",
                    size.width,
                    size.height
                );

                let shape = glam::vec2(size.width as f32, size.height as f32);
                let pixel_shape = glam::Vec2::ONE / shape;
                let viewport_shape = 2. * shape / shape.y;

                let update_time = time::Duration::from_secs_f64(1. / crate::UPDATE_RATE);
                let pixels_per_frame = NonZeroUsize::new({
                    let mut rng = rand_pcg::Pcg32::from_entropy();
                    let start = time::Instant::now();
                    let color = multisampled_color(
                        &input,
                        &mut rng,
                        glam::Vec2::ZERO,
                        glam::Vec2::ONE,
                        glam::Vec2::splat(2.),
                        crate::SAMPLES_PER_PIXEL,
                    );
                    std::hint::black_box(color);
                    let elapsed = start.elapsed();
                    update_time.div_duration_f64(elapsed).floor() as usize
                })
                .unwrap_or(NonZeroUsize::new(1).unwrap());

                match (0..size.height).into_par_iter().try_for_each_init(
                    || {
                        (
                            rand_pcg::Pcg32::from_entropy(),
                            vec![[0; 4]; size.width].into_boxed_slice(),
                            pixels_per_frame.clone(),
                        )
                    },
                    |(ref mut rng, ref mut row_buffer, ref mut pixels_per_frame), row| {
                        let y = shape.y - row as f32 - 1.;
                        let mut column_range = 0..pixels_per_frame.get().min(size.width);
                        loop {
                            let start = time::Instant::now();
                            for (column, out) in
                                row_buffer[column_range.clone()].iter_mut().enumerate()
                            {
                                let xy = glam::vec2(column as f32, y);
                                let uv = xy / shape;
                                *out = multisampled_color(
                                    &input,
                                    rng,
                                    uv,
                                    pixel_shape,
                                    viewport_shape,
                                    crate::SAMPLES_PER_PIXEL,
                                );
                            }

                            let elapsed = start.elapsed();
                            *pixels_per_frame = NonZeroUsize::new(
                                (column_range.len() as f64 * update_time.div_duration_f64(elapsed))
                                    .floor() as usize,
                            )
                            .unwrap_or(NonZeroUsize::new(1).unwrap());

                            log::trace!("Flushing pixels at row {row}, columns {column_range:?}");
                            let mut pixels = state.pixels.lock();
                            if continue_.strong_count() == 0 {
                                return Err(RenderError::Cancel);
                            }
                            let frame = pixels.get_frame();
                            if frame.len() != size.width * size.height * 4 {
                                return Err(RenderError::Resize);
                            }
                            let (frame, _) = frame.as_chunks_mut::<4>();
                            let row_out = &mut frame[row * size.width..][..size.width];
                            row_out[column_range.clone()]
                                .copy_from_slice(&row_buffer[column_range.clone()]);

                            column_range =
                                column_range.end..column_range.end + pixels_per_frame.get();

                            if column_range.end > size.width {
                                return Ok(());
                            }
                        }
                    },
                ) {
                    Ok(()) => {
                        log::info!("Renderer thread finished in {:?}", start.elapsed());
                        state.window.request_redraw();
                    }
                    Err(RenderError::Cancel) => log::info!("Render cancelled"),
                    Err(RenderError::Resize) => {
                        log::warn!("Renderer thread detected a resize, cancelling render")
                    }
                }
            }
        }));
        Self { thread, continue_ }
    }

    pub fn restart(&mut self, state: Arc<State>) -> thread::Result<()> {
        self.break_join()?;
        *self = Handle::new(state);
        Ok(())
    }

    pub fn is_running(&mut self) -> thread::Result<bool> {
        if self.thread.is_some() {
            let is_running = Arc::weak_count(&self.continue_) != 0;

            if !is_running {
                self.thread.take().unwrap().join()?;
            }

            Ok(is_running)
        } else {
            Ok(false)
        }
    }

    pub fn break_join(&mut self) -> thread::Result<()> {
        if let Some(thread) = self.thread.take() {
            log::info!("Stopping thread {:?}", thread.thread());
            self.continue_ = Arc::new(());
            thread.join()?;
        }
        Ok(())
    }
}

enum RenderError {
    Cancel,
    Resize,
}

fn multisampled_color(
    input: &raytracer::Input,
    rng: &mut rand_pcg::Pcg32,
    uv: glam::Vec2,
    pixel_shape: glam::Vec2,
    viewport_shape: glam::Vec2,
    samples: usize,
) -> [u8; 4] {
    let sum = (0..samples)
        .map(|_| {
            let uv = uv + glam::vec2(rng.gen(), rng.gen()) * pixel_shape;
            let ray = raytracer::Ray {
                origin: crate::ORIGIN,
                direction: crate::ORIGIN
                    + glam::Vec3::from((
                        (uv - glam::Vec2::splat(0.5)) * viewport_shape,
                        -crate::FOCAL_LENGTH,
                    )),
            };

            raytracer::color(input, rng, &ray, crate::MAX_DEPTH)
                .clamp(glam::Vec3::ZERO, glam::Vec3::ONE)
        })
        .reduce(|acc, c| acc + c)
        .unwrap_or(glam::Vec3::ZERO);
    let avg = sum / samples as f32;
    linear_to_srgb(glam::Vec4::from((avg, 1.)))
}

fn linear_to_srgb(color: glam::Vec4) -> [u8; 4] {
    color.to_array().map(|c| {
        let s = if c <= 0.0031308 {
            12.92 * c
        } else {
            1.055 * c.powf(1. / 2.4) - 0.055
        };
        (s * 256.).clamp(0., 255.) as u8
    })
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use test::Bencher;

    #[bench]
    fn multisample(b: &mut Bencher) {
        let primitives = crate::example_primitives();
        let input = raytracer::Input {
            primitives: &primitives,
        };
        let mut rng = rand_pcg::Pcg32::from_entropy();

        b.iter(|| {
            super::multisampled_color(
                &input,
                &mut rng,
                glam::Vec2::ZERO,
                glam::Vec2::ONE,
                glam::Vec2::splat(2.),
                1024,
            )
        });
    }
}
