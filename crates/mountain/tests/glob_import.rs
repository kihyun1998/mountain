//! `use mountain::*` beside the real winit crate keeps `winit::` meaning winit.

use mountain::*;

#[test]
fn a_glob_import_leaves_the_winit_crate_reachable() {
    let size = winit::dpi::LogicalSize::new(640.0, 480.0);
    assert_eq!(size.width, 640.0);
    let _ = PhysicalSize {
        width: 1,
        height: 1,
    };
}
