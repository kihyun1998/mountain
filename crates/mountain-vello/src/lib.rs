//! A renderer Backend that draws the Core's frames with Vello on wgpu.

use mountain_core::{Color, FrameOutcome, PhysicalRect, PhysicalSize, Renderer};
use vello::kurbo::Affine;
use vello::peniko::Fill;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::{self, CurrentSurfaceTexture, SurfaceTarget};
use vello::{AaConfig, RenderParams, RendererOptions, Scene};

pub use vello::Error;

/// Draws onto one window's surface.
pub struct VelloRenderer {
    context: RenderContext,
    surface: RenderSurface<'static>,
    renderer: vello::Renderer,
    scene: Scene,
}

impl VelloRenderer {
    /// Creates a surface for `target` and a GPU device that can present to it.
    pub fn new(
        target: impl Into<SurfaceTarget<'static>>,
        size: PhysicalSize,
    ) -> Result<Self, Error> {
        let mut context = RenderContext::new();
        let surface = pollster::block_on(context.create_surface(
            target,
            size.width.max(1),
            size.height.max(1),
            wgpu::PresentMode::AutoVsync,
        ))?;
        let renderer = vello::Renderer::new(
            &context.devices[surface.dev_id].device,
            RendererOptions::default(),
        )?;
        Ok(VelloRenderer {
            context,
            surface,
            renderer,
            scene: Scene::new(),
        })
    }

    fn present(&mut self) -> Result<FrameOutcome, Error> {
        let device = &self.context.devices[self.surface.dev_id];
        let params = RenderParams {
            base_color: vello::peniko::Color::TRANSPARENT,
            width: self.surface.config.width,
            height: self.surface.config.height,
            antialiasing_method: AaConfig::Msaa16,
        };
        self.renderer.render_to_texture(
            &device.device,
            &device.queue,
            &self.scene,
            &self.surface.target_view,
            &params,
        )?;
        let acquired = self.surface.surface.get_current_texture();
        let stale = match acquired {
            CurrentSurfaceTexture::Success(texture) => Ok(texture),
            CurrentSurfaceTexture::Suboptimal(texture) => {
                drop(texture);
                Err(())
            }
            CurrentSurfaceTexture::Outdated => Err(()),
            CurrentSurfaceTexture::Occluded => return Ok(FrameOutcome::Occluded),
            CurrentSurfaceTexture::Timeout => return Ok(FrameOutcome::Retry),
            CurrentSurfaceTexture::Lost => panic!("the window's surface was lost"),
            CurrentSurfaceTexture::Validation => {
                panic!("validation error getting the surface texture")
            }
        };
        let texture = match stale {
            Ok(texture) => texture,
            Err(()) => {
                self.context.configure_surface(&self.surface);
                match self.surface.surface.get_current_texture() {
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
        self.surface.blitter.copy(
            &device.device,
            &mut encoder,
            &self.surface.target_view,
            &view,
        );
        device.queue.submit([encoder.finish()]);
        device.queue.present(texture);
        device
            .device
            .poll(wgpu::PollType::Poll)
            .expect("polling the GPU device failed");
        Ok(FrameOutcome::Presented)
    }
}

impl Renderer for VelloRenderer {
    fn begin_frame(&mut self, size: PhysicalSize) {
        let config = &self.surface.config;
        if (config.width, config.height) != (size.width, size.height) {
            self.context
                .resize_surface(&mut self.surface, size.width, size.height);
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

    fn end_frame(&mut self) -> FrameOutcome {
        match self.present() {
            Ok(outcome) => outcome,
            Err(e) => panic!("rendering a frame failed: {e}"),
        }
    }
}
