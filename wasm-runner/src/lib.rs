use raytracer::{
    winit::{event_loop::EventLoop, platform::web::EventLoopExtWebSys},
    App,
};
use wasm_bindgen::prelude::*;

#[derive(serde::Deserialize, Clone, Copy, Debug)]
#[serde(default)]
pub struct Args {
    pub width: u32,
    pub height: u32,
    pub sample_count: u32,
    pub ray_depth: u32,
    pub max_framebuffer_weight: f32,
}

impl From<Args> for raytracer::Args {
    fn from(args: Args) -> Self {
        raytracer::Args {
            width: args.width,
            height: args.height,
            samples_per_frame: args.sample_count,
            ray_depth: args.ray_depth,
            max_framebuffer_weight: args.max_framebuffer_weight,
        }
    }
}

impl From<raytracer::Args> for Args {
    fn from(args: raytracer::Args) -> Self {
        Args {
            width: args.width,
            height: args.height,
            sample_count: args.samples_per_frame,
            ray_depth: args.ray_depth,
            max_framebuffer_weight: args.max_framebuffer_weight,
        }
    }
}

impl Default for Args {
    fn default() -> Self {
        raytracer::Args::default().into()
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    #[derive(serde::Deserialize, Clone, Copy, Debug)]
    #[serde(default)]
    struct QueryArgs {
        log_level: log::Level,
    }

    impl Default for QueryArgs {
        fn default() -> Self {
            QueryArgs {
                log_level: log::Level::Info,
            }
        }
    }

    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    let query = query_string();
    let args: QueryArgs = serde_urlencoded::from_str(&query).expect("Parsing query string");
    console_log::init_with_level(args.log_level).expect("Initializing logger");
    log::debug!("Parsed args from query: {args:?}");
}

#[wasm_bindgen]
pub fn spawn_app(canvas: web_sys::HtmlCanvasElement, args: JsValue) -> Result<(), JsValue> {
    let args: Args = if args.is_undefined() {
        let query = query_string();
        serde_urlencoded::from_str(&query).expect("Parsing query string")
    } else {
        serde_wasm_bindgen::from_value(args).map_err(|e| JsError::new(&format!("{:?}", e)))?
    };

    let event_loop = EventLoop::with_user_event()
        .build()
        .expect("failed to build an event loop");
    let app = App::new(&event_loop, args.into(), raytracer::PlatformArgs { canvas });
    event_loop.spawn_app(app);
    Ok(())
}

fn query_string() -> String {
    let query = web_sys::window().unwrap().location().search().unwrap();
    Some(query.as_str())
        .filter(|q| q.is_empty())
        .or_else(|| query.strip_prefix('?'))
        .map(|s| s.to_owned())
        .unwrap()
}
