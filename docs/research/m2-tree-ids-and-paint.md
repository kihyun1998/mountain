# M2: node ids, paint and headless test drivers in other frameworks

Gathered for the M2 design session (2026-10-29) from each project's default branch, by reading source files and official docs. Paths are `repo: path`. Rows marked *not read* rest on memory or inference and need confirming against the source before anyone relies on them.

## Node ids and tree storage

| Project | Id and how it is made | Reused after removal? | Separate key? | Tree storage |
|---|---|---|---|---|
| Masonry | `WidgetId(NonZeroU64)` from a static `AtomicU64` counter | No | Yes, tags (`widget_tag.rs`) | `tree_arena` crate, `HashMap`s keyed by `u64` |
| Dioxus | `ElementId(usize)`, an index into a `slab::Slab` | Yes, no generation check | Yes, vnode `key()`; all siblings keyed or none | `slab` |
| Floem | `ViewId`, a `slotmap` key | Slot reused with a new version | No | `slotmap` |
| GPUI | `EntityId` (slotmap key); `ElementId` enum chosen by the user, made global as a path of ancestor ids | Entity: slotmap versioning | `ElementId` is the key | slotmap for entities; elements rebuilt each frame |
| Iced | `widget::Id`: `Unique(usize)` from a counter, or `Custom(str)` | No (counter) | Diffing is by type tag and position, not `Id` | Owned recursive `Tree` |
| egui | `Id(NonZeroU64)`, a hash of a source and the parent id | Not allocated; same source gives the same id | The id is built from user salts | No retained tree |
| Freya | `NodeId(NonZeroU64)` from a counter | No | Yes, `DiffKey` | `FxHashMap`s |
| React | Fibers, no numeric id (*not read*: `ReactFiber.js`) | — | `key`; old fibers mapped by key or index (`ReactChildFiber.js`) | Linked fibers |
| Flutter | Object identity (*not read*) | — | `Key`, `GlobalKey` (*not read*) | Object tree |

Sources: linebender/xilem `masonry_core/src/core/widget.rs`, `tree_arena/src/lib.rs`; DioxusLabs/dioxus `packages/vdom/src/arena.rs`, `scopes.rs`, `diff/iterator.rs` (the core crate moved from `packages/core`); lapce/floem `src/view/id.rs`; zed-industries/zed `crates/gpui/src/app/entity_map.rs`, `src/element.rs`; iced-rs/iced `core/src/widget/id.rs`, `core/src/widget/tree.rs`; emilk/egui `crates/egui/src/id.rs`; marc2332/freya `crates/freya-core/src/node_id.rs`, `tree.rs`, `diff_key.rs`; facebook/react `packages/react-reconciler/src/ReactChildFiber.js`.

AccessKit (`accesskit/src/lib.rs`): `NodeId` is "the stable identity of a `Node`, unique within the node's tree". A `TreeUpdate` "will overwrite any existing node with the same ID", and a removed child is dropped by updating its parent's `children`. No rule on reusing a removed id was found.

## Paint

| Project | Direct or recorded | Caching | How a node paints | Coordinates |
|---|---|---|---|---|
| Masonry | Recorded: a scene per widget, joined with `append_transformed` | Per widget (`scene_cache`), redrawn on `request_paint` | `Widget::paint`, trait objects | Local; parent applies the transform |
| anyrender | `PaintScene` trait; `recording::Scene` stores a command enum and replays it | Caller's choice | Calls on the trait | An `Affine` on every call |
| GPUI | Recorded into a `Scene` of `Primitive`s | Whole tree; `.cached()` views reuse paint | `Element::paint`, trait objects | Absolute bounds; scaled when a primitive is built |
| Iced | Direct calls on `Renderer` | Whole tree (from the trait shape) | `Widget::draw` | Absolute bounds (inferred, *not read*) |
| Flutter | Recorded into `PictureRecorder`s inside a layer tree | Per repaint boundary (`isRepaintBoundary`, `markNeedsPaint`) | `RenderObject.paint(context, offset)` | Parent passes an offset |
| Slint | Direct through `ItemRenderer`, with a layer cache | Per item (`ItemCache`), cleared on scale change | `Item::render`, a closed set of built-in items | Local; `translate(item_origin)` |
| Freya | Direct calls on a Skia canvas | Whole tree (`// TODO: Use incremental rendering`) | `ElementExt::render`, trait objects | Absolute areas; scale applied per element |

Sources: linebender/xilem `masonry_core/src/core/widget.rs`, `src/passes/paint.rs`; DioxusLabs/anyrender `crates/anyrender/src/lib.rs`, `src/recording.rs`; zed `crates/gpui/src/scene.rs`, `src/window.rs`, `src/view.rs`; iced `core/src/renderer.rs`, `core/src/widget.rs`; flutter `packages/flutter/lib/src/rendering/object.dart`; slint `internal/core/item_rendering.rs`; freya `crates/freya-core/src/render_pipeline.rs`.

## Headless test drivers

| Project | Runs without | Asserts on | Packaging |
|---|---|---|---|
| Masonry `masonry_testing` | Window and GPU (renders with `imaging_vello_cpu`) | Widget tree, AccessKit tree, recorded widget calls, PNG snapshots | Separate crate |
| React `react-test-renderer` | DOM | JSON tree of host instances; deprecated (react.dev/warnings/react-test-renderer) | Separate package |
| Flutter `flutter_test` | Real window | Element tree, golden PNGs (`matchesGoldenFile`) | SDK package |
| Dioxus | Any renderer | The list of mutations (`rebuild_to_vec().edits`, v0.6.3 `packages/core/tests/create_dom.rs`) | Tests inside the core crate |
| Iced `iced_test` | Window | Messages; snapshots by image or hash | Separate crate |
| egui `egui_kittest` | Window; GPU optional | AccessKit nodes; PNG snapshots; its README prefers plain or `insta` tests over images | Separate crate |
| GPUI | OS platform (`TestPlatform`, `TestWindow`) | App and view state | Module of `gpui` |
