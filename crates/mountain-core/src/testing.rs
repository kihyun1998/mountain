//! A platform and a renderer for driving the Core with no window and no GPU.

use crate::{
    Color, Core, FrameOutcome, PhysicalRect, PhysicalSize, Platform, Renderer, WindowEvent,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    FillRect(PhysicalRect, Color),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub size: PhysicalSize,
    pub commands: Vec<Command>,
}

/// Stores draw commands, one [`Frame`] per `begin_frame`/`end_frame` pair.
#[derive(Default)]
pub struct RecordingRenderer {
    pub frames: Vec<Frame>,
    /// Outcomes to report for the next frames, in order; then `Presented`.
    pub outcomes: Vec<FrameOutcome>,
}

impl Renderer for RecordingRenderer {
    fn begin_frame(&mut self, size: PhysicalSize) {
        self.frames.push(Frame {
            size,
            commands: Vec::new(),
        });
    }
    fn fill_rect(&mut self, rect: PhysicalRect, color: Color) {
        self.frames
            .last_mut()
            .expect("fill_rect outside a frame")
            .commands
            .push(Command::FillRect(rect, color));
    }
    fn end_frame(&mut self) -> FrameOutcome {
        if self.outcomes.is_empty() {
            FrameOutcome::Presented
        } else {
            self.outcomes.remove(0)
        }
    }
}

/// Feeds a fixed list of events, answering each redraw request with a
/// `RedrawRequested` as a windowing system would.
pub struct TestPlatform {
    events: Vec<WindowEvent>,
    pub renderer: RecordingRenderer,
    pub exited: bool,
}

impl TestPlatform {
    pub fn new(events: Vec<WindowEvent>) -> TestPlatform {
        TestPlatform {
            events,
            renderer: RecordingRenderer::default(),
            exited: false,
        }
    }
}

impl Platform for TestPlatform {
    type Error = std::convert::Infallible;

    fn run(&mut self, core: &mut Core) -> Result<(), Self::Error> {
        for event in std::mem::take(&mut self.events) {
            let response = core.handle(event, &mut self.renderer);
            let mut redraw = response.redraw;
            for _ in 0..100 {
                if !redraw {
                    break;
                }
                redraw = core
                    .handle(WindowEvent::RedrawRequested, &mut self.renderer)
                    .redraw;
            }
            assert!(!redraw, "the Core kept asking for redraws");
            if response.exit {
                self.exited = true;
                return Ok(());
            }
        }
        Ok(())
    }
}
