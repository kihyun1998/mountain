//! What an app author uses: the Core's types, and with the default `vello`
//! and `winit` features, the Vello renderer and the winit platform.

pub use mountain_core::{
    Color, ColoredBox, Core, DisplayList, FrameOutcome, HostNode, LogicalRect, LogicalSize, NodeId,
    PhysicalRect, PhysicalSize, Platform, RenderError, Renderer, Response, Tree, WindowEvent,
};

#[cfg(feature = "vello")]
pub use mountain_vello::VelloRenderer;
#[cfg(feature = "winit")]
pub use mountain_winit::WinitPlatform;

/// The backends this crate re-exports, each behind the feature of its name.
pub mod backend {
    /// The Vello renderer backend.
    #[cfg(feature = "vello")]
    pub use mountain_vello as vello;

    /// The winit platform backend.
    #[cfg(feature = "winit")]
    pub use mountain_winit as winit;
}
