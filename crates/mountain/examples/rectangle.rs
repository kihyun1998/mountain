//! A window on a white background with a red box holding a blue one, redrawn
//! on resize.

use mountain::{Color, ColoredBox, Core, LogicalRect, Platform, VelloRenderer, WinitPlatform};

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
    let blue = Color {
        r: 40,
        g: 80,
        b: 220,
        a: 255,
    };
    let mut core = Core::new().with_background(white);
    let root = core.tree().root();
    let outer = core.tree_mut().add(
        root,
        LogicalRect {
            x: 40.0,
            y: 40.0,
            width: 240.0,
            height: 160.0,
        },
        ColoredBox::new(red),
    );
    core.tree_mut().add(
        outer,
        LogicalRect {
            x: 20.0,
            y: 20.0,
            width: 80.0,
            height: 60.0,
        },
        ColoredBox::new(blue),
    );
    let mut platform = WinitPlatform::new("mountain: rectangle", (640.0, 480.0), |window, size| {
        VelloRenderer::new(window, size)
    });
    if let Err(e) = platform.run(&mut core) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
