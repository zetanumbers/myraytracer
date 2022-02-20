#![feature(slice_as_chunks)]

use std::time;

use raytracer_common::{render_fn, RawRenderer, Renderer};

#[no_mangle]
pub unsafe extern "C" fn new(width: usize, height: usize) -> RawRenderer {
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
                let x = i % width;
                let y = i / width;

                frame[i] = [
                    (255 * x / (width - 1)) as u8,
                    (255 * y / (height - 1)) as u8,
                    63,
                    255,
                ];
            }

            let batch_finish = time::Instant::now();
            let elapsed = batch_finish - batch_start;
            batch_start = batch_finish;

            approx_pixels = if elapsed.is_zero() {
                approx_pixels * 32
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
