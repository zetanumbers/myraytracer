use std::{
    mem,
    path::{Path, PathBuf},
    sync::mpsc,
    time,
};

use notify::Watcher;
use raytracer_common::{RawRenderer, Renderer};

use crate::wnt;

pub struct RendererPlugin {
    path: PathBuf,
    _watcher: notify::RecommendedWatcher,
    notifications: mpsc::Receiver<notify::DebouncedEvent>,
    changed: bool,
    inner: Option<RendererModule>,
}

struct RendererModule {
    _cdylib: libloading::Library,
    renderer: mem::ManuallyDrop<Renderer>,
}

impl Drop for RendererModule {
    fn drop(&mut self) {
        unsafe { mem::ManuallyDrop::drop(&mut self.renderer) }
    }
}

impl RendererPlugin {
    pub fn new(path: &Path) -> Self {
        let path = path.canonicalize().expect("Canonicalizing renerer's path");
        let (tx, notifications) = mpsc::channel();

        let mut watcher =
            notify::watcher(tx, time::Duration::from_secs(3)).expect("Initializing file watcher");

        watcher
            .watch(
                &path.parent().expect("Got root as a renderer plugin path"),
                notify::RecursiveMode::Recursive,
            )
            .expect("Watching renderer");

        RendererPlugin {
            path,
            _watcher: watcher,
            notifications,
            changed: false,
            inner: None,
        }
    }

    pub unsafe fn load(&mut self, size: wnt::PhysicalSize<u32>) {
        log::info!(
            "Loading render plugin with size {}x{}: {}",
            size.width,
            size.height,
            self.path.display()
        );

        self.unload();
        self.changed();
        self.changed = false;

        let module = libloading::Library::new(&self.path).expect("Loading renderer");

        let symbol: libloading::Symbol<'_, unsafe extern "C" fn(usize, usize) -> RawRenderer> =
            module.get(b"new\0").expect("Getting `new` symbol");

        let renderer = mem::ManuallyDrop::new(Renderer::from_raw(symbol(
            size.width as usize,
            size.height as usize,
        )));

        self.inner = Some(RendererModule {
            _cdylib: module,
            renderer,
        });
    }

    pub fn unload(&mut self) {
        self.inner = None;
    }

    pub fn render(&mut self, frame: &mut [u8]) -> bool {
        self.inner
            .as_mut()
            .expect("Renderer isn't loaded")
            .renderer
            .render(frame)
    }

    pub fn changed(&mut self) -> bool {
        loop {
            use notify::DebouncedEvent::*;
            let ev = self.notifications.try_recv();
            let ev = match ev {
                Ok(ev) => ev,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => unreachable!(),
            };
            let is_relevant = |p: &PathBuf| matches!(p.canonicalize(), Ok(p) if p == self.path);

            match &ev {
                Create(p) | NoticeWrite(p) | Write(p) | Chmod(p) | Remove(p) | NoticeRemove(p) => {
                    if is_relevant(p) {
                        log::debug!("Got relevant watcher event: {ev:?}");
                        self.changed = true
                    }
                }

                Rename(src, dest) => {
                    if [src, dest].into_iter().any(is_relevant) {
                        log::debug!("Got relevant watcher event: {ev:?}");
                        self.changed = true
                    }
                }

                Error(e, p) => {
                    panic!("Watcher error occured at {p:?}: {e}")
                }

                ev => panic!("Unhandled watch event: {ev:?}"),
            }
        }

        self.changed
    }
}
