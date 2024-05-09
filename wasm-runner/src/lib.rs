use wasm_bindgen::prelude::*;

#[derive(serde::Deserialize, Clone, Copy, Debug)]
#[serde(default)]
pub struct Args {
    pub width: u32,
    pub height: u32,
    pub sample_count: u32,
    pub ray_depth: u32,
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

impl From<raytracer::Args> for Args {
    fn from(args: raytracer::Args) -> Self {
        Args {
            width: args.width,
            height: args.height,
            sample_count: args.sample_count,
            ray_depth: args.ray_depth,
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
pub async fn run(canvas: web_sys::HtmlCanvasElement, args: JsValue) -> Result<(), JsValue> {
    let args: Args = if args.is_undefined() {
        let query = query_string();
        serde_urlencoded::from_str(&query).expect("Parsing query string")
    } else {
        args.into_serde()
            .map_err(|e| JsError::new(&format!("{:?}", e)))?
    };

    raytracer::run(args.into(), raytracer::PlatformArgs { canvas }).await;

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
