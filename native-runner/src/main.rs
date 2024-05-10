use clap::Parser as _;
use raytracer::{winit::event_loop::EventLoop, App};

fn main() {
    // TODO: use tracing?
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();
    let args = Args::parse();
    let event_loop = EventLoop::with_user_event()
        .build()
        .expect("failed to build an event loop");
    let mut app = App::new(&event_loop, args.into(), raytracer::PlatformArgs {});
    event_loop.run_app(&mut app).expect("failed to run an app");
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(target_arch = "wasm32"), derive(clap::Parser))]
pub struct Args {
    #[clap(long, default_value_t = 0)]
    width: u32,
    #[clap(long, default_value_t = 0)]
    height: u32,
    #[clap(long, default_value_t = 100)]
    sample_count: u32,
    #[clap(long, default_value_t = 50)]
    ray_depth: u32,
}

impl From<Args> for raytracer::Args {
    fn from(args: Args) -> Self {
        raytracer::Args {
            width: args.width,
            height: args.height,
            sample_count: args.sample_count,
            ray_depth: args.ray_depth,
        }
    }
}
