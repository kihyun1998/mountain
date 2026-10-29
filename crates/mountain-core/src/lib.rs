//! The Core: what is drawn and when, behind traits for the renderer and the
//! platform. It depends on no rendering, text or windowing crate.

pub mod testing;
mod tree;

use tree::DrawCommand;
pub use tree::{ColoredBox, DisplayList, HostNode, NodeId, Tree};

/// A surface size in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}

/// A size in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalSize {
    pub width: f64,
    pub height: f64,
}

/// A rectangle in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl LogicalRect {
    /// The same rectangle in physical pixels at `scale_factor`.
    pub fn to_physical(self, scale_factor: f64) -> PhysicalRect {
        PhysicalRect {
            x: self.x * scale_factor,
            y: self.y * scale_factor,
            width: self.width * scale_factor,
            height: self.height * scale_factor,
        }
    }
}

/// A rectangle in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// An sRGB colour with straight alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Draws one frame at a time onto a surface. A frame's size is never zero.
pub trait Renderer {
    fn begin_frame(&mut self, size: PhysicalSize);
    fn fill_rect(&mut self, rect: PhysicalRect, color: Color);
    fn end_frame(&mut self) -> Result<FrameOutcome, RenderError>;
}

/// Why a renderer could not draw a frame, such as a lost GPU device.
pub type RenderError = Box<dyn std::error::Error + Send + Sync>;

/// What became of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameOutcome {
    /// The frame reached the screen, or there was nothing to draw.
    Presented,
    /// The window is occluded; the frame is drawn again once it is visible.
    Occluded,
    /// The surface could not take the frame now; it is drawn again at once.
    Retry,
}

/// What a platform reports about the window the Core draws into.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowEvent {
    Created {
        size: PhysicalSize,
        scale_factor: f64,
    },
    Resized(PhysicalSize),
    ScaleFactorChanged {
        scale_factor: f64,
        size: PhysicalSize,
    },
    /// Whether the window is hidden from view, minimised or covered.
    Occluded(bool),
    RedrawRequested,
    CloseRequested,
}

/// What the Core asks of the platform after an event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Response {
    pub redraw: bool,
    pub exit: bool,
}

/// Turns a windowing system's events into [`WindowEvent`]s, hands each to
/// [`Core::handle`] with a renderer for the window, and acts on the
/// [`Response`]. A platform backend does not pick the renderer: it is given a
/// way to make one for each window it creates.
pub trait Platform {
    type Error;
    fn run(&mut self, core: &mut Core) -> Result<(), Self::Error>;
}

/// Holds the tree of Host nodes and the surface it is drawn on.
pub struct Core {
    background: Option<Color>,
    tree: Tree,
    size: Option<PhysicalSize>,
    scale_factor: f64,
    occluded: bool,
    /// A frame is owed: one was asked for but did not reach the screen.
    pending: bool,
}

impl Default for Core {
    fn default() -> Core {
        Core::new()
    }
}

impl Core {
    pub fn new() -> Core {
        Core {
            background: None,
            tree: Tree::new(),
            size: None,
            scale_factor: 1.0,
            occluded: false,
            pending: false,
        }
    }

    /// Fills the whole surface with `color` before the tree.
    pub fn with_background(mut self, color: Color) -> Core {
        self.background = Some(color);
        self
    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub fn tree_mut(&mut self) -> &mut Tree {
        &mut self.tree
    }

    pub fn handle(
        &mut self,
        event: WindowEvent,
        renderer: &mut dyn Renderer,
    ) -> Result<Response, RenderError> {
        let response = match event {
            WindowEvent::Created { size, scale_factor } => {
                self.size = Some(size);
                self.scale_factor = scale_factor;
                Response {
                    redraw: true,
                    exit: false,
                }
            }
            WindowEvent::Resized(size) => {
                self.size = Some(size);
                Response {
                    redraw: true,
                    exit: false,
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, size } => {
                self.scale_factor = scale_factor;
                self.size = Some(size);
                Response {
                    redraw: true,
                    exit: false,
                }
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                Response {
                    redraw: !occluded && self.pending,
                    exit: false,
                }
            }
            WindowEvent::RedrawRequested => {
                if self.occluded {
                    self.pending = true;
                    return Ok(Response::default());
                }
                let outcome = self.draw(renderer)?;
                self.pending = outcome == FrameOutcome::Occluded;
                Response {
                    redraw: outcome == FrameOutcome::Retry,
                    exit: false,
                }
            }
            WindowEvent::CloseRequested => Response {
                redraw: false,
                exit: true,
            },
        };
        Ok(response)
    }

    fn draw(&mut self, renderer: &mut dyn Renderer) -> Result<FrameOutcome, RenderError> {
        let Some(size) = self.size.filter(|s| s.width > 0 && s.height > 0) else {
            return Ok(FrameOutcome::Presented);
        };
        renderer.begin_frame(size);
        if let Some(color) = self.background {
            let whole = PhysicalRect {
                x: 0.0,
                y: 0.0,
                width: f64::from(size.width),
                height: f64::from(size.height),
            };
            renderer.fill_rect(whole, color);
        }
        let scale_factor = self.scale_factor;
        self.tree.paint(&mut |list, x, y| {
            for command in list.commands() {
                match *command {
                    DrawCommand::FillRect(rect, color) => {
                        let placed = LogicalRect {
                            x: rect.x + x,
                            y: rect.y + y,
                            ..rect
                        };
                        renderer.fill_rect(placed.to_physical(scale_factor), color);
                    }
                }
            }
        });
        renderer.end_frame()
    }
}
