use mountain_core::testing::{Command, Frame, RecordingRenderer, TestPlatform};
use mountain_core::{Color, Core, PhysicalSize, Platform, Rect, Response, WindowEvent};

const RED: Color = Color {
    r: 255,
    g: 0,
    b: 0,
    a: 255,
};

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn size(width: u32, height: u32) -> PhysicalSize {
    PhysicalSize { width, height }
}

fn core() -> Core {
    Core::new().with_rect(rect(10.0, 10.0, 100.0, 50.0), RED)
}

fn run(events: Vec<WindowEvent>) -> TestPlatform {
    let mut core = core();
    let mut platform = TestPlatform::new(events);
    let Ok(()) = platform.run(&mut core);
    platform
}

#[test]
fn creating_then_resizing_draws_the_rectangle_at_each_size() {
    let platform = run(vec![
        WindowEvent::Created {
            size: size(800, 600),
            scale_factor: 1.0,
        },
        WindowEvent::Resized(size(1024, 768)),
    ]);
    let drawn = Command::FillRect(rect(10.0, 10.0, 100.0, 50.0), RED);
    assert_eq!(
        platform.renderer.frames,
        [
            Frame {
                size: size(800, 600),
                commands: vec![drawn.clone()]
            },
            Frame {
                size: size(1024, 768),
                commands: vec![drawn]
            },
        ]
    );
}

#[test]
fn close_asks_the_platform_to_exit_and_draws_nothing() {
    let mut core = core();
    let mut renderer = RecordingRenderer::default();
    let response = core.handle(WindowEvent::CloseRequested, &mut renderer);
    assert_eq!(
        response,
        Response {
            redraw: false,
            exit: true
        }
    );
    assert!(renderer.frames.is_empty());
}

#[test]
fn the_test_platform_stops_at_close() {
    let platform = run(vec![
        WindowEvent::Created {
            size: size(800, 600),
            scale_factor: 1.0,
        },
        WindowEvent::CloseRequested,
        WindowEvent::Resized(size(1024, 768)),
    ]);
    assert!(platform.exited);
    assert_eq!(platform.renderer.frames.len(), 1);
}

#[test]
fn the_rectangle_is_scaled_to_physical_pixels() {
    let platform = run(vec![
        WindowEvent::Created {
            size: size(1600, 1200),
            scale_factor: 2.0,
        },
        WindowEvent::ScaleFactorChanged {
            scale_factor: 1.5,
            size: size(1200, 900),
        },
    ]);
    assert_eq!(
        platform.renderer.frames,
        [
            Frame {
                size: size(1600, 1200),
                commands: vec![Command::FillRect(rect(20.0, 20.0, 200.0, 100.0), RED)],
            },
            Frame {
                size: size(1200, 900),
                commands: vec![Command::FillRect(rect(15.0, 15.0, 150.0, 75.0), RED)],
            },
        ]
    );
}

#[test]
fn a_zero_sized_surface_is_not_drawn() {
    let platform = run(vec![
        WindowEvent::Created {
            size: size(0, 0),
            scale_factor: 1.0,
        },
        WindowEvent::Resized(size(800, 0)),
        WindowEvent::Resized(size(800, 600)),
    ]);
    let sizes: Vec<_> = platform.renderer.frames.iter().map(|f| f.size).collect();
    assert_eq!(sizes, [size(800, 600)]);
}
