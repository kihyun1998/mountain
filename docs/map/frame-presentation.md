# Frame presentation

How a frame the Core asks for reaches the screen, and what happens when it does not. Code: `Renderer::end_frame` and `FrameOutcome` and the `pending` flag in `crates/mountain-core`, `VelloRenderer::present` in `crates/mountain-vello`, the `Occluded` mapping in `crates/mountain-winit`.

## Why three outcomes

A renderer that cannot present a frame says why, and the Core decides when to try again:

| wgpu 30 `get_current_texture` | `FrameOutcome` | The Core |
|---|---|---|
| `Success` | `Presented` | nothing owed |
| `Occluded` | `Occluded` | owes a frame, draws it when `WindowEvent::Occluded(false)` arrives |
| `Timeout`, or `Outdated`/`Suboptimal` still failing after reconfiguring | `Retry` | asks for a redraw at once |
| `Lost` | `Retry`, after the surface is created again for the same window | asks for a redraw at once |
| `Validation`, with the device lost | error | returns it; the platform ends `run` with it |
| `Validation`, device not lost | panics | — |

Two outcomes, not one, because the platforms differ:

- wgpu returns `Occluded` only on Metal (`wgpu-hal-30.0.1/src/metal/surface.rs:368`), and winit 0.30.13 sends `Occluded` events on macOS, X11, web and iOS, but not on Windows or Wayland.
- Retrying at once on `Occluded`, as Vello's own example does (`research/examples/simple/src/main.rs` at v0.11.0), would spin the event loop on macOS while the window is hidden, since Metal answers `Occluded` immediately. wgpu's docs say to skip and try again once the window is no longer occluded (`wgpu-30.0.1/src/api/surface_texture.rs`).
- On Vulkan and DX12 a frame is lost to `Timeout` (a one-second acquire) or to a failed reconfigure, and no visibility event follows, so waiting would leave the old frame up.

## Errors and panics

A renderer reports a failure it did not cause as a `RenderError` from `end_frame`; the Core returns it from `handle`, and the winit platform ends `run` with `Error::Renderer`. The renderer factory's error ends `run` the same way. These are errors: no device or surface could be made, recreating a lost surface failed, Vello failed to render, polling the device failed, or the device was lost (its lost callback fired).

`Validation` with the device intact stays a panic: wgpu's default error handler already panics on validation errors before `get_current_texture` returns (`wgpu-30.0.1/src/backend/wgpu_core.rs`, `default_error_handler`), so reaching that arm means our use of wgpu is wrong, which is a bug to fix rather than a condition to report. A lost device is not recovered: the error ends `run`.

## Measured

On macOS at scale 2, with only "skip on Occluded" and no repayment: a window resized by script while behind another window kept the previous frame, stretched to the new size (rectangle at 112, 118, 675 × 218 px instead of 80, 144, 480 × 320). The log showed both new-size frames returning `Occluded`, then `Occluded(false)` on bringing the window forward, with nothing redrawn. With the `pending` repayment the same steps give the rectangle at 80, 144, 480 × 320.

## Traps

- `Suboptimal` carries a live `SurfaceTexture`. Configuring the surface while it is alive panics in wgpu (`api/surface.rs`: "panics if a SurfaceTexture is alive"); a `Suboptimal(_)` pattern does not move it out, so it lives to the end of the `match`. Drop it first. Only Vulkan returns `Suboptimal`; Vello's example has the same pattern.
- A lost device arrives as `CurrentSurfaceTexture::Validation`, not as an error of its own: wgpu drops errors of type `DeviceLost` (they are "surfaced via callback") and reports the failed acquire as `SurfaceStatus::Validation` (`wgpu-30.0.1/src/backend/wgpu_core.rs`). The renderer checks its device-lost flag before treating `Validation` as a bug.
- Recreating a lost surface must drop the old one first. Vello's `create_surface` configures the new surface at once, which makes a swapchain; Vulkan refuses a second swapchain on the same window (`ERROR_NATIVE_WINDOW_IN_USE_KHR`, `wgpu-hal-30.0.1/src/vulkan/swapchain/native.rs`), and wgpu turns the failure into a validation error that panics. DX12 maps a failed swapchain creation the same way (`dx12/mod.rs`); that a live swapchain on the window is what fails it there was not checked.
- Each device gets its own lost flag, so a callback from a device the renderer has moved off cannot fail frames on the new one.
- `Renderer::begin_frame` never sees a zero size: the Core does not draw a zero-sized surface, because a minimised window reports 0 × 0 and Vello's `resize_surface` panics on zero.

## Not tried

- Any run on Windows or Linux; those are built in CI only.

## Live resize on macOS

Dragging the window's corner made the rectangle shake for as long as the drag lasted (seen by hand, 2026-10-29): during a live resize the window changes size inside a Core Animation transaction while Metal presented frames on its own schedule. The Vello renderer now sets `presentsWithTransaction` on the surface's `CAMetalLayer` for frames at a new size and clears it on the first frame at an unchanged size; the same drag then showed no shake.

- It lives in the renderer because winit 0.30 tells the app nothing about a live resize: `windowWillStartLiveResize:` and `windowDidEndLiveResize:` only adjust resize increments (`winit-0.30.13/src/platform_impl/macos/window_delegate.rs`). A size change between frames is the signal the renderer already has, so no trait carries one. After a drag ends the flag stays set until the next frame; that costs speed, not correctness.
- The layer flag alone is enough. wgpu-hal 30 reads `presentsWithTransaction()` when it acquires a texture and, when set, presents after `waitUntilScheduled` (`wgpu-hal-30.0.1/src/metal/surface.rs`, `metal/mod.rs`). Masonry reconfigures the surface after changing the flag (`masonry_winit` `vello_util.rs`, `on_window_resize_state_change`); with wgpu 30 that is not needed, so the renderer does not.
- Reaching the layer goes through `Surface::as_hal::<Metal>()`, an `unsafe` function in wgpu 30; that call is the repository's only `unsafe` block.
