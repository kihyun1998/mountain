//! A window on a white background with a red rectangle, redrawn on resize.

use mountain::{Color, Core, LogicalRect, Platform};
use mountain_vello::VelloRenderer;
use mountain_winit::WinitPlatform;

fn main() {
    let white = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    let red = Color {
        r: 220,
        g: 40,
        b: 40,
        a: 255,
    };
    let mut core = Core::new().with_background(white).with_rect(
        LogicalRect {
            x: 40.0,
            y: 40.0,
            width: 240.0,
            height: 160.0,
        },
        red,
    );
    let mut platform = WinitPlatform::new("mountain: rectangle", (640.0, 480.0), |window, size| {
        VelloRenderer::new(window, size)
    });
    if let Err(e) = platform.run(&mut core) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
