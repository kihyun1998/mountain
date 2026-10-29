# Each Host node caches its Display list

Each Host node keeps the Display list it recorded when it last painted, in its own coordinates and in logical pixels, and records it again only when its own values or its size change, since its size is what it paints at; a frame joins the kept lists, offsetting each by its position in its parent, and the Core converts to physical pixels when it replays the result to the `Renderer`. Moving a node, reordering siblings or a new scale factor therefore re-records nothing. Keeping one recorded scene per widget is Masonry's approach (a `scene_cache`, redrawn when the widget requests paint; `masonry_core/src/passes/paint.rs` in linebender/xilem, read 2026-10-29), but Masonry, like GPUI, paints everything again on a new scale factor (`request_render_all` on `WindowEvent::Rescale` in `masonry_core/src/app/render_root.rs`); leaving lists alone across a scale change matches Flutter, which swaps only the root transform (`rendering/view.dart`). The size rule and the corrected sources were added on 2026-10-29 after #15, with the maintainer's agreement; the decision itself did not change. It was put in before any measurement showed a need: the maintainer's call (M2 design session, 2026-10-29), so it is theirs to reverse.

## Considered options

- **Repaint the whole tree every frame, and add a cache once a measurement asks for one**: Freya does this, with a `// TODO: Use incremental rendering` in `crates/freya-core/src/render_pipeline.rs`; GPUI does it by default and reuses paint only for views marked `.cached()`. Rejected: adding the cache later would mean laying a "needs repaint" path through the tree then.
- **Flutter's repaint boundaries**: only nodes marked `isRepaintBoundary` keep their own layer, and `markNeedsPaint` re-records up to the nearest one (`packages/flutter/lib/src/rendering/object.dart`). Rejected for now: it needs a rule for where boundaries go, which M2 has nothing to base on.

## Consequences

Not measured: the cost of joining many small lists each frame, and the memory a list per node takes. If either shows up, boundaries are the alternative already looked at.
