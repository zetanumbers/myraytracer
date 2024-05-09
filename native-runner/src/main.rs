use clap::Parser as _;
use pollster::FutureExt as _;

fn main() {
    // TODO: use tracing?
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();
    let args = Args::parse();
    raytracer::run(args.into(), raytracer::PlatformArgs {}).block_on()
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(target_arch = "wasm32"), derive(clap::Parser))]
pub struct Args {
    #[clap(long, default_value_t = 400)]
    width: u32,
    #[clap(long, default_value_t = 225)]
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
