//! A platform Backend that runs the Core in a winit window.

use std::sync::Arc;

use mountain_core::{Core, PhysicalSize, Platform, Renderer, Response, WindowEvent};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent as WinitEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

pub use winit::window::Window as WinitWindow;

/// Why the window could not be run.
#[derive(Debug)]
pub enum Error {
    EventLoop(winit::error::EventLoopError),
    Window(winit::error::OsError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::EventLoop(e) => write!(f, "the event loop failed: {e}"),
            Error::Window(e) => write!(f, "creating the window failed: {e}"),
        }
    }
}

impl std::error::Error for Error {}

/// Opens one window and feeds its events to the Core. `make_renderer` is
/// called once the window exists, with the window to draw into and its size
/// in physical pixels. `run` has to be called on the main thread, and once
/// per process: winit allows one event loop.
pub struct WinitPlatform<F> {
    title: String,
    size: (f64, f64),
    make_renderer: F,
}

impl<R, F> WinitPlatform<F>
where
    R: Renderer,
    F: FnMut(Arc<Window>, PhysicalSize) -> R,
{
    /// `size` is the window's inner size in logical pixels.
    pub fn new(title: impl Into<String>, size: (f64, f64), make_renderer: F) -> Self {
        WinitPlatform {
            title: title.into(),
            size,
            make_renderer,
        }
    }
}

impl<R, F> Platform for WinitPlatform<F>
where
    R: Renderer,
    F: FnMut(Arc<Window>, PhysicalSize) -> R,
{
    type Error = Error;

    fn run(&mut self, core: &mut Core) -> Result<(), Error> {
        let event_loop = EventLoop::new().map_err(Error::EventLoop)?;
        let mut app = App {
            platform: self,
            core,
            window: None,
            error: None,
        };
        event_loop.run_app(&mut app).map_err(Error::EventLoop)?;
        app.error.map_or(Ok(()), Err)
    }
}

struct App<'a, F, R> {
    platform: &'a mut WinitPlatform<F>,
    core: &'a mut Core,
    window: Option<(Arc<Window>, R)>,
    error: Option<Error>,
}

fn physical(size: winit::dpi::PhysicalSize<u32>) -> PhysicalSize {
    PhysicalSize {
        width: size.width,
        height: size.height,
    }
}

impl<F, R> App<'_, F, R>
where
    R: Renderer,
{
    fn send(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        let Some((window, renderer)) = &mut self.window else {
            return;
        };
        let Response { redraw, exit } = self.core.handle(event, renderer);
        if redraw {
            window.request_redraw();
        }
        if exit {
            event_loop.exit();
        }
    }
}

impl<F, R> ApplicationHandler for App<'_, F, R>
where
    R: Renderer,
    F: FnMut(Arc<Window>, PhysicalSize) -> R,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let (width, height) = self.platform.size;
        let attributes = Window::default_attributes()
            .with_title(self.platform.title.clone())
            .with_inner_size(LogicalSize::new(width, height));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(e) => {
                self.error = Some(Error::Window(e));
                event_loop.exit();
                return;
            }
        };
        let size = physical(window.inner_size());
        let renderer = (self.platform.make_renderer)(window.clone(), size);
        let created = WindowEvent::Created {
            size: physical(window.inner_size()),
            scale_factor: window.scale_factor(),
        };
        self.window = Some((window, renderer));
        self.send(event_loop, created);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WinitEvent) {
        let Some((window, _)) = &self.window else {
            return;
        };
        if window.id() != id {
            return;
        }
        let event = match event {
            WinitEvent::Resized(size) => WindowEvent::Resized(physical(size)),
            WinitEvent::ScaleFactorChanged { scale_factor, .. } => {
                WindowEvent::ScaleFactorChanged {
                    scale_factor,
                    size: physical(window.inner_size()),
                }
            }
            WinitEvent::Occluded(occluded) => WindowEvent::Occluded(occluded),
            WinitEvent::RedrawRequested => WindowEvent::RedrawRequested,
            WinitEvent::CloseRequested => WindowEvent::CloseRequested,
            _ => return,
        };
        self.send(event_loop, event);
    }
}
