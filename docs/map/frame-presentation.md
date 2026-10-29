# Frame presentation

How a frame the Core asks for reaches the screen, and what happens when it does not. Code: `Renderer::end_frame` and `FrameOutcome` and the `pending` flag in `crates/mountain-core`, `VelloRenderer::present` in `crates/mountain-vello`, the `Occluded` mapping in `crates/mountain-winit`.

## Why three outcomes

A renderer that cannot present a frame says why, and the Core decides when to try again:

| wgpu 30 `get_current_texture` | `FrameOutcome` | The Core |
|---|---|---|
| `Success` | `Presented` | nothing owed |
| `Occluded` | `Occluded` | owes a frame, draws it when `WindowEvent::Occluded(false)` arrives |
| `Timeout`, or `Outdated`/`Suboptimal` still failing after reconfiguring | `Retry` | asks for a redraw at once |

Two outcomes, not one, because the platforms differ:

- wgpu returns `Occluded` only on Metal (`wgpu-hal-30.0.1/src/metal/surface.rs:368`), and winit 0.30.13 sends `Occluded` events on macOS, X11, web and iOS, but not on Windows or Wayland.
- Retrying at once on `Occluded`, as Vello's own example does (`research/examples/simple/src/main.rs` at v0.11.0), would spin the event loop on macOS while the window is hidden, since Metal answers `Occluded` immediately. wgpu's docs say to skip and try again once the window is no longer occluded (`wgpu-30.0.1/src/api/surface_texture.rs`).
- On Vulkan and DX12 a frame is lost to `Timeout` (a one-second acquire) or to a failed reconfigure, and no visibility event follows, so waiting would leave the old frame up.

## Measured

On macOS at scale 2, with only "skip on Occluded" and no repayment: a window resized by script while behind another window kept the previous frame, stretched to the new size (rectangle at 112, 118, 675 × 218 px instead of 80, 144, 480 × 320). The log showed both new-size frames returning `Occluded`, then `Occluded(false)` on bringing the window forward, with nothing redrawn. With the `pending` repayment the same steps give the rectangle at 80, 144, 480 × 320.

## Traps

- `Suboptimal` carries a live `SurfaceTexture`. Configuring the surface while it is alive panics in wgpu (`api/surface.rs`: "panics if a SurfaceTexture is alive"); a `Suboptimal(_)` pattern does not move it out, so it lives to the end of the `match`. Drop it first. Only Vulkan returns `Suboptimal`; Vello's example has the same pattern.
- `Renderer::begin_frame` never sees a zero size: the Core does not draw a zero-sized surface, because a minimised window reports 0 × 0 and Vello's `resize_surface` panics on zero.

## Not tried

- Live drag-resize on macOS with the window in front. Masonry turns on `presents_with_transaction` during a live resize to keep frames in step with the window (`masonry_winit` `vello_util.rs`, `event_loop_runner.rs`).
- Any run on Windows or Linux; those are built in CI only.
