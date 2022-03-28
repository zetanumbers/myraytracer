use wasm_bindgen::{prelude::*, JsCast};

pub struct CanvasWindow {
    raw_handle: raw_window_handle::WebHandle,
    _canvas: web_sys::HtmlCanvasElement,
}

impl CanvasWindow {
    pub fn new(width: u32, height: u32) -> Result<Self, JsValue> {
        let window = web_sys::window().expect("no window found");
        let document = window.document().expect("no document found");

        static LAST_ID: parking_lot::Mutex<u32> = parking_lot::const_mutex(0);

        let mut l = LAST_ID.lock();
        let id = loop {
            *l = l.checked_add(1).expect("Too many canvas windows");

            if document
                .query_selector(&format!(r#"canvas[data-raw-handle="{}"]"#, *l))?
                .is_none()
            {
                break *l;
            }
        };
        drop(l);

        let body = document.body().expect("no body found");
        let canvas: web_sys::HtmlCanvasElement =
            document.create_element("canvas")?.unchecked_into();
        canvas.set_width(width);
        canvas.set_height(height);
        canvas.set_attribute("data-raw-handle", &format!("{id}"))?;
        body.append_child(&canvas)?;

        let mut raw_handle = raw_window_handle::WebHandle::empty();
        raw_handle.id = id;

        Ok(Self {
            raw_handle,
            _canvas: canvas,
        })
    }
}

unsafe impl raw_window_handle::HasRawWindowHandle for CanvasWindow {
    fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        raw_window_handle::RawWindowHandle::Web(self.raw_handle)
    }
}
