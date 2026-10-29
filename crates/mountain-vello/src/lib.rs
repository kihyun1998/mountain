//! A renderer Backend that draws the Core's frames with Vello on wgpu.

use std::sync::{Arc, Mutex};

use mountain_core::{Color, FrameOutcome, PhysicalRect, PhysicalSize, RenderError, Renderer};
use vello::kurbo::Affine;
use vello::peniko::Fill;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::{self, CurrentSurfaceTexture, SurfaceTarget};
use vello::{AaConfig, RenderParams, RendererOptions, Scene};

pub use vello::Error;

/// Set to the reason once a device's lost callback has fired.
type DeviceLost = Arc<Mutex<Option<String>>>;

/// Draws onto one window's surface, keeping the window and the surface's
/// last configuration.
pub struct VelloRenderer {
    context: RenderContext,
    /// `None` only after recreating a lost surface failed.
    surface: Option<RenderSurface<'static>>,
    config: wgpu::SurfaceConfiguration,
    dev_id: usize,
    renderer: vello::Renderer,
    scene: Scene,
    target: Box<dyn Fn() -> SurfaceTarget<'static> + Send>,
    /// Set by the current device's lost callback.
    device_lost: DeviceLost,
}

impl VelloRenderer {
    /// Creates a surface for `target` and a GPU device that can present to it.
    pub fn new<T>(target: T, size: PhysicalSize) -> Result<Self, Error>
    where
        T: Clone + Into<SurfaceTarget<'static>> + Send + 'static,
    {
        let mut context = RenderContext::new();
        let target: Box<dyn Fn() -> SurfaceTarget<'static> + Send> =
            Box::new(move || target.clone().into());
        let surface = pollster::block_on(context.create_surface(
            target(),
            size.width.max(1),
            size.height.max(1),
            wgpu::PresentMode::AutoVsync,
        ))?;
        let (renderer, device_lost) = Self::renderer_for(&context, surface.dev_id)?;
        Ok(VelloRenderer {
            config: surface.config.clone(),
            dev_id: surface.dev_id,
            surface: Some(surface),
            context,
            renderer,
            scene: Scene::new(),
            target,
            device_lost,
        })
    }

    /// A Vello renderer on device `dev_id`, and the flag that device's lost
    /// callback sets.
    fn renderer_for(
        context: &RenderContext,
        dev_id: usize,
    ) -> Result<(vello::Renderer, DeviceLost), Error> {
        let device = &context.devices[dev_id].device;
        let device_lost: DeviceLost = Arc::new(Mutex::new(None));
        let flag = device_lost.clone();
        device.set_device_lost_callback(move |reason, message| {
            *flag.lock().unwrap() = Some(format!("{reason:?}: {message}"));
        });
        let renderer = vello::Renderer::new(device, RendererOptions::default())?;
        Ok((renderer, device_lost))
    }

    /// Creates the surface again for the same window and configuration. The
    /// old surface is dropped first, before a new swapchain is made.
    fn recreate_surface(&mut self) -> Result<(), Error> {
        self.surface = None;
        let surface = self.context.instance.create_surface((self.target)())?;
        let surface = pollster::block_on(self.context.create_render_surface(
            surface,
            self.config.width,
            self.config.height,
            self.config.present_mode,
        ))?;
        if surface.dev_id != self.dev_id {
            let (renderer, device_lost) = Self::renderer_for(&self.context, surface.dev_id)?;
            self.renderer = renderer;
            self.device_lost = device_lost;
            self.dev_id = surface.dev_id;
        }
        self.config = surface.config.clone();
        self.surface = Some(surface);
        Ok(())
    }

    fn lost_device(&self) -> Option<RenderError> {
        let reason = self.device_lost.lock().unwrap().clone()?;
        Some(format!("the GPU device was lost ({reason})").into())
    }

    fn present(&mut self) -> Result<FrameOutcome, RenderError> {
        let Some(surface) = self.surface.as_mut() else {
            return Err("the window's surface is gone".into());
        };
        let device = &self.context.devices[surface.dev_id];
        let params = RenderParams {
            base_color: vello::peniko::Color::TRANSPARENT,
            width: surface.config.width,
            height: surface.config.height,
            antialiasing_method: AaConfig::Msaa16,
        };
        self.renderer.render_to_texture(
            &device.device,
            &device.queue,
            &self.scene,
            &surface.target_view,
            &params,
        )?;
        let acquired = surface.surface.get_current_texture();
        let stale = match acquired {
            CurrentSurfaceTexture::Success(texture) => Ok(texture),
            CurrentSurfaceTexture::Suboptimal(texture) => {
                drop(texture);
                Err(())
            }
            CurrentSurfaceTexture::Outdated => Err(()),
            CurrentSurfaceTexture::Occluded => return Ok(FrameOutcome::Occluded),
            CurrentSurfaceTexture::Timeout => return Ok(FrameOutcome::Retry),
            CurrentSurfaceTexture::Lost => {
                self.recreate_surface()?;
                return Ok(FrameOutcome::Retry);
            }
            CurrentSurfaceTexture::Validation => match self.lost_device() {
                Some(error) => return Err(error),
                None => panic!("validation error getting the surface texture"),
            },
        };
        let texture = match stale {
            Ok(texture) => texture,
            Err(()) => {
                self.context.configure_surface(surface);
                match surface.surface.get_current_texture() {
                    CurrentSurfaceTexture::Success(texture) => texture,
                    _ => return Ok(FrameOutcome::Retry),
                }
            }
        };
        let mut encoder = device
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mountain blit"),
            });
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        surface
            .blitter
            .copy(&device.device, &mut encoder, &surface.target_view, &view);
        device.queue.submit([encoder.finish()]);
        device.queue.present(texture);
        if let Err(e) = device.device.poll(wgpu::PollType::Poll) {
            return Err(format!("polling the GPU device failed: {e}").into());
        }
        Ok(FrameOutcome::Presented)
    }
}

impl Renderer for VelloRenderer {
    fn begin_frame(&mut self, size: PhysicalSize) {
        if let Some(surface) = self.surface.as_mut()
            && (surface.config.width, surface.config.height) != (size.width, size.height)
        {
            self.context
                .resize_surface(surface, size.width, size.height);
            self.config = surface.config.clone();
        }
        self.scene.reset();
    }

    fn fill_rect(&mut self, rect: PhysicalRect, color: Color) {
        let shape =
            vello::kurbo::Rect::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height);
        let brush = vello::peniko::Color::from_rgba8(color.r, color.g, color.b, color.a);
        self.scene
            .fill(Fill::NonZero, Affine::IDENTITY, brush, None, &shape);
    }

    fn end_frame(&mut self) -> Result<FrameOutcome, RenderError> {
        if let Some(error) = self.lost_device() {
            return Err(error);
        }
        self.present()
    }
}
