use std::{env, time};

use itertools::Itertools;
// use rayon::prelude::*;
const REDRAW_PIXELS: usize = 1024 * 1024;

fn main() {
    env_logger::init();

    let init_args = raytracer::RendererInitArgs::deserialize(
        &env::args().nth(1).expect("Could not obtain init args"),
    );
    let size = init_args.size;
    let sender = init_args.command_sender();

    log::info!("Start of render, size: `{size:?}`");
    let start = time::Instant::now();

    let mut batch_count = 0;
    let batch_count = &mut batch_count;
    (0..size[1] as usize)
        .rev()
        .cartesian_product(0..size[0] as usize)
        // .par_bridge()
        // .for_each_with((sender, 0), |(sender, batch_count), (j, i)| {
        .for_each(|(j, i)| {
            let red = (i as f32 / (size[0] - 1) as f32 * 255.0) as u8;
            let green = (j as f32 / (size[1] - 1) as f32 * 255.0) as u8;
            let blue = 63;

            sender
                .send(raytracer::Command::Set {
                    pos: [i, j],
                    color: [red, green, blue],
                })
                .unwrap();

            *batch_count += 1;
            if *batch_count == REDRAW_PIXELS {
                *batch_count = 0;
                sender.send(raytracer::Command::Redraw).unwrap();
            }
        });
    let secs = start.elapsed().as_secs_f32();
    log::info!("Rendered in {secs} seconds of size: {size:?}");
}
