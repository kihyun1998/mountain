use mountain_core::testing::{Command, Frame, RecordingRenderer, TestPlatform};
use mountain_core::{
    Color, Core, FrameOutcome, LogicalRect, PhysicalRect, PhysicalSize, Platform, Response,
    WindowEvent,
};

const RED: Color = Color {
    r: 255,
    g: 0,
    b: 0,
    a: 255,
};

fn logical(x: f64, y: f64, width: f64, height: f64) -> LogicalRect {
    LogicalRect {
        x,
        y,
        width,
        height,
    }
}

fn physical(x: f64, y: f64, width: f64, height: f64) -> PhysicalRect {
    PhysicalRect {
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
    Core::new().with_rect(logical(10.0, 10.0, 100.0, 50.0), RED)
}

fn run(events: Vec<WindowEvent>) -> TestPlatform {
    let mut core = core();
    let mut platform = TestPlatform::new(events);
    platform.run(&mut core).unwrap();
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
    let drawn = Command::FillRect(physical(10.0, 10.0, 100.0, 50.0), RED);
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
    let response = core
        .handle(WindowEvent::CloseRequested, &mut renderer)
        .unwrap();
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
                commands: vec![Command::FillRect(physical(20.0, 20.0, 200.0, 100.0), RED)],
            },
            Frame {
                size: size(1200, 900),
                commands: vec![Command::FillRect(physical(15.0, 15.0, 150.0, 75.0), RED)],
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

#[test]
fn a_background_fills_the_whole_surface_before_the_content() {
    let white = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    let mut core = core().with_background(white);
    let mut platform = TestPlatform::new(vec![WindowEvent::Created {
        size: size(800, 600),
        scale_factor: 2.0,
    }]);
    platform.run(&mut core).unwrap();
    assert_eq!(
        platform.renderer.frames[0].commands,
        [
            Command::FillRect(physical(0.0, 0.0, 800.0, 600.0), white),
            Command::FillRect(physical(20.0, 20.0, 200.0, 100.0), RED),
        ]
    );
}

#[test]
fn a_frame_lost_to_occlusion_is_drawn_again_once_the_window_is_visible() {
    let mut core = core();
    let mut platform = TestPlatform::new(vec![
        WindowEvent::Created {
            size: size(800, 600),
            scale_factor: 1.0,
        },
        WindowEvent::Occluded(false),
    ]);
    platform.renderer.outcomes = vec![FrameOutcome::Occluded];
    platform.run(&mut core).unwrap();
    let sizes: Vec<_> = platform.renderer.frames.iter().map(|f| f.size).collect();
    assert_eq!(sizes, [size(800, 600), size(800, 600)]);
}

#[test]
fn nothing_is_drawn_while_the_window_is_occluded() {
    let mut core = core();
    let mut platform = TestPlatform::new(vec![
        WindowEvent::Created {
            size: size(800, 600),
            scale_factor: 1.0,
        },
        WindowEvent::Occluded(true),
        WindowEvent::Resized(size(900, 300)),
        WindowEvent::Resized(size(1000, 400)),
        WindowEvent::Occluded(false),
    ]);
    platform.run(&mut core).unwrap();
    let sizes: Vec<_> = platform.renderer.frames.iter().map(|f| f.size).collect();
    assert_eq!(sizes, [size(800, 600), size(1000, 400)]);
}

#[test]
fn becoming_visible_with_nothing_pending_draws_nothing() {
    let platform = run(vec![
        WindowEvent::Created {
            size: size(800, 600),
            scale_factor: 1.0,
        },
        WindowEvent::Occluded(true),
        WindowEvent::Occluded(false),
    ]);
    assert_eq!(platform.renderer.frames.len(), 1);
}

#[test]
fn a_frame_the_renderer_asks_to_retry_is_drawn_again_at_once() {
    let mut core = core();
    let mut platform = TestPlatform::new(vec![WindowEvent::Created {
        size: size(800, 600),
        scale_factor: 1.0,
    }]);
    platform.renderer.outcomes = vec![FrameOutcome::Retry];
    platform.run(&mut core).unwrap();
    assert_eq!(platform.renderer.frames.len(), 2);
}

#[test]
fn a_frame_lost_to_occlusion_is_not_retried_while_occluded() {
    let mut core = core();
    let mut platform = TestPlatform::new(vec![WindowEvent::Created {
        size: size(800, 600),
        scale_factor: 1.0,
    }]);
    platform.renderer.outcomes = vec![FrameOutcome::Occluded];
    platform.run(&mut core).unwrap();
    assert_eq!(platform.renderer.frames.len(), 1);
}

#[test]
fn a_logical_rectangle_converts_to_physical_pixels_at_a_fractional_scale() {
    let converted = logical(10.0, 4.0, 100.0, 50.0).to_physical(1.5);
    assert_eq!(converted, physical(15.0, 6.0, 150.0, 75.0));
}

#[test]
fn a_renderer_error_is_returned_by_handle() {
    let mut core = core();
    let mut renderer = RecordingRenderer::default();
    core.handle(
        WindowEvent::Created {
            size: size(800, 600),
            scale_factor: 1.0,
        },
        &mut renderer,
    )
    .unwrap();
    renderer.fail_next = Some("device lost".to_string());
    let error = core
        .handle(WindowEvent::RedrawRequested, &mut renderer)
        .unwrap_err();
    assert_eq!(error.to_string(), "device lost");
}

#[test]
fn a_renderer_error_ends_the_run_with_that_error() {
    let mut core = core();
    let mut platform = TestPlatform::new(vec![
        WindowEvent::Created {
            size: size(800, 600),
            scale_factor: 1.0,
        },
        WindowEvent::Resized(size(1024, 768)),
    ]);
    platform.renderer.fail_next = Some("device lost".to_string());
    let error = platform.run(&mut core).unwrap_err();
    assert_eq!(error.to_string(), "device lost");
    assert_eq!(platform.renderer.frames.len(), 1);
}
