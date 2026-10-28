# Rust UI framework: what to decide first, what to build first

Checked on 2026-10-28. Repository metadata (latest release, last push, archived flag) was read from the GitHub API on that date. Source links to React and Flutter are pinned to the commits read (`react/react@dc9aed3`, `flutter/flutter@7102630`); links into Rust projects point at their default branch as of that date. The Rust UI ecosystem changes quickly, so re-check versions before relying on them.

## Summary

### Decisions, in the order they block each other

1. **Target platforms.** Desktop only, desktop plus web, or desktop plus mobile. This decides whether "render to the DOM" is an option at all and how much platform-embedding work there is.
2. **Who draws the pixels.** Own the pixels with a 2D renderer crate (Vello, Skia, tiny-skia) or hand drawing to the browser DOM or a webview. Drawing to the DOM removes the layout and text decisions (the browser does both); owning the pixels makes them the two largest pieces of work. A Flutter-like framework owns the pixels.
3. **Reactivity and state model.** Re-run components and diff a light tree (React, Flutter, Dioxus, Xilem), fine-grained signals (Leptos, Floem, Slint), Elm messages (Iced), immediate mode (egui), or GPUI-style entities. This is the project's stated core ("built-in state management") and it decides the shape of every widget API, so it is fixed before any widget is written.
4. **Retained or immediate.** Mostly follows from 3. Retained suits Flutter-style layout and accessibility; egui's own README lists layout as the main cost of immediate mode.
5. **Layout model.** Flutter box constraints written in-house, CSS Flexbox/Grid through Taffy, or both (Taffy's low-level API can run inside your own tree).
6. **Text stack.** Parley (pairs with Vello), cosmic-text, or Skia's paragraph module (comes with Skia). Building one in-house is not a realistic early option.
7. **Accessibility.** AccessKit is the de facto choice; the early decision is to give every node a stable identity that can become an AccessKit node.
8. **API surface.** Plain builder functions or a macro DSL (`rsx!`, `view!`, a separate `.slint`-style language). Can start as builders and add a macro later.
9. **Component library model.** A shadcn-style "copy the source into your crate" CLI only makes sense after the primitives API (3, 5, 8) has settled.

Windowing is close to a non-decision: winit is used by Xilem, Iced, egui, Freya, Floem, Slint's winit backend and Dioxus native; only GPUI writes its own platform layer.

### Build order for the first milestones

| # | Milestone | Reuse or build | Depends on |
|---|---|---|---|
| M0 | Write decisions 1 to 3 down as ADRs | build | nothing |
| M1 | Window opens, clears, draws a filled rounded rect | reuse winit + wgpu + Vello (or Skia) | 1, 2 |
| M2 | Retained node tree with stable ids, a paint pass, and a headless test driver | build | 4 |
| M3 | Layout: box constraints, `Row`/`Column`/`Padding`/`SizedBox` | build (or reuse Taffy) | 5 |
| M4 | A text label measured by layout and drawn | reuse Parley (or Skia paragraph) | 6 |
| M5 | Pointer input, hit testing, a `Button` with a click callback | build | M2, M3 |
| M6 | Counter app: state change triggers rebuild and minimal update | build (the core of the project) | 3 |
| M7 | Counter app exposed through AccessKit | reuse AccessKit, build the tree mapping | 7 |
| M8 | Then: focus and keyboard, text input with IME, scrolling, theming, component library | mixed | 8, 9 |

The order M1 to M7 is a judgement call, not a fact. The main alternative is to build M6 (state and reconciliation) first against a headless test host, the way React's reconciler runs against a host config and `react-test-renderer` exists. That order tests the stated core first but delays seeing pixels. See section 3.

## 1. How React and Flutter split the work

### React

- **Reconciler versus renderer.** React's diffing and scheduling live in `react-reconciler`; each platform supplies a "host config" that says how to create and mutate host nodes. The package README states: "A 'host config' is an object that you need to provide, and that describes how to make something happen in the 'host' environment (e.g. DOM, canvas, console, or whatever your rendering target is)". The repository ships several host configs: DOM ([`ReactFiberConfigDOM.js`](https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-dom-bindings/src/client/ReactFiberConfigDOM.js)), ART ([`ReactFiberConfigART.js`](https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-art/src/ReactFiberConfigART.js)), and a test host ([`ReactFiberConfigTestHost.js`](https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-test-renderer/src/ReactFiberConfigTestHost.js)). The reconciler supports a "mutation mode" and a "persistent mode" ([README](https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-reconciler/README.md)). Note: `github.com/facebook/react` now resolves to `react/react` (observed through the GitHub API on 2026-10-28).
- **Render and commit.** React's docs split each update into trigger, render and commit, and state that "React only changes the DOM nodes if there's a difference between renders" ([react.dev, Render and Commit](https://react.dev/learn/render-and-commit)). The work loop is [`ReactFiberWorkLoop.js`](https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-reconciler/src/ReactFiberWorkLoop.js), child diffing is [`ReactChildFiber.js`](https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-reconciler/src/ReactChildFiber.js), and the commit phase is [`ReactFiberCommitWork.js`](https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-reconciler/src/ReactFiberCommitWork.js).
- **Hooks and state.** Hook state is stored per component instance, in call order: "Hooks are stored as a linked list on the fiber's memoizedState field" ([`ReactFiberHooks.js`, line 266](https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-reconciler/src/ReactFiberHooks.js#L266)); the `Hook` type has `memoizedState`, `baseState`, `baseQueue`, `queue`, `next` ([line 197](https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-reconciler/src/ReactFiberHooks.js#L197)). This is why the order of hook calls must not change between renders.
- **Scheduler.** A separate `scheduler` package runs work in time slices and yields to the host; `frameYieldMs = 5` ([`SchedulerFeatureFlags.js`](https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/scheduler/src/SchedulerFeatureFlags.js)), `shouldYieldToHost()` in [`Scheduler.js`](https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/scheduler/src/forks/Scheduler.js). Priorities are modelled as lanes ([`ReactFiberLane.js`](https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-reconciler/src/ReactFiberLane.js)).

What carries over to Rust: the reconciler/host split (keep the state and diff core independent of the drawing backend, and write a test host early). What does not carry over directly: React does not do layout or text itself; the DOM does.

### Flutter

- **Three trees.** Widgets are immutable configuration; Elements are the persistent instances; RenderObjects do layout and paint. "The element tree is persistent from frame to frame ... allowing Flutter to act as if the widget hierarchy is fully disposable while caching its underlying representation" ([architectural overview](https://docs.flutter.dev/resources/architectural-overview)). In source: `Widget`, `StatelessWidget`, `StatefulWidget`, `InheritedWidget`, `RenderObjectWidget` in [`widgets/framework.dart`](https://github.com/flutter/flutter/blob/710263041fe6f956e7a328756c63d751cd27f98a/packages/flutter/lib/src/widgets/framework.dart).
- **Reconciliation rule.** An element is reused when `oldWidget.runtimeType == newWidget.runtimeType && oldWidget.key == newWidget.key` ([`Widget.canUpdate`, line 384](https://github.com/flutter/flutter/blob/710263041fe6f956e7a328756c63d751cd27f98a/packages/flutter/lib/src/widgets/framework.dart#L384)).
- **State.** `State.setState` runs the callback and calls `_element!.markNeedsBuild()` ([line 1164](https://github.com/flutter/flutter/blob/710263041fe6f956e7a328756c63d751cd27f98a/packages/flutter/lib/src/widgets/framework.dart#L1164)). Data passed down the tree uses `InheritedWidget` and `BuildContext.dependOnInheritedWidgetOfExactType`, which registers a dependency so dependents rebuild when it changes (same file, line 1862).
- **Layout protocol.** "Constraints go down. Sizes go up. Parent sets position." ([Understanding constraints](https://docs.flutter.dev/ui/layout/constraints)). The overview describes it as a single depth-first walk that lays out "in O(n) time" ([architectural overview](https://docs.flutter.dev/resources/architectural-overview)). In source: `BoxConstraints` ([`rendering/box.dart`](https://github.com/flutter/flutter/blob/710263041fe6f956e7a328756c63d751cd27f98a/packages/flutter/lib/src/rendering/box.dart)), `RenderObject.layout(Constraints constraints, {bool parentUsesSize = false})`, `markNeedsLayout`, `markNeedsPaint`, and `PipelineOwner.flushLayout` / `flushCompositingBits` with relayout and repaint boundaries ([`rendering/object.dart`](https://github.com/flutter/flutter/blob/710263041fe6f956e7a328756c63d751cd27f98a/packages/flutter/lib/src/rendering/object.dart)). The documented limits of this model: a widget cannot pick any size it wants and does not know its own position ([constraints page](https://docs.flutter.dev/ui/layout/constraints)).
- **Framework / engine / embedder.** The framework composites the widget tree into a scene; the engine rasterizes it and exposes `dart:ui`; the embedder supplies surfaces, input, accessibility and the event loop ([architectural overview](https://docs.flutter.dev/resources/architectural-overview); [The Engine architecture](https://github.com/flutter/flutter/blob/710263041fe6f956e7a328756c63d751cd27f98a/docs/about/The-Engine-architecture.md)). The engine now lives in the same repository under [`engine/src/flutter`](https://github.com/flutter/flutter/tree/710263041fe6f956e7a328756c63d751cd27f98a/engine/src/flutter).
- **Rendering backend.** Impeller is the only renderer on iOS, the default on Android API 29+, and the default on macOS, Linux and Windows "as of Flutter 3.47"; web uses Skia ([Impeller docs](https://docs.flutter.dev/perf/impeller)).
- **Text.** The engine's paragraph layout wraps Skia's paragraph module ([`engine/src/flutter/txt/src/skia/paragraph_skia.cc`](https://github.com/flutter/flutter/tree/710263041fe6f956e7a328756c63d751cd27f98a/engine/src/flutter/txt/src/skia)).

What carries over to Rust: the three-tree split, the constraints protocol, dirty flags with relayout/repaint boundaries, and the engine/embedder split. Flutter shows that owning the pixels means owning layout, text, accessibility and input as well.

## 2. The early decisions

### 2.1 Comparison table

| # | Decision | Options | What each option forces later | Blocks |
|---|---|---|---|---|
| 1 | Target platforms | (a) desktop native; (b) + web wasm; (c) + mobile | (a) winit covers it. (b) needs a canvas/WebGPU path (Vello GPU has a WebGL2 backend; wgpu targets WebGPU) or a separate DOM backend like `xilem_web`. (c) needs Android/iOS lifecycles, IME and AccessKit's Android/iOS adapters; Dioxus, Slint and egui ship mobile support, most others do not | 2, 6, 7 |
| 2 | Who draws | (a) own pixels with Vello; (b) own pixels with Skia (`skia-safe`); (c) tiny-skia / vello_cpu (CPU); (d) DOM or webview | (a) pure Rust, wgpu; pairs with Parley; Vello's own README now calls `vello_cpu` the more mature path and `vello_gpu` the intended primary GPU path. (b) mature, brings its paragraph module for text, but a large C++ build. (c) simple, slower. (d) layout, text, IME and accessibility come from the browser; decisions 5 and 6 go away; result resembles Leptos/Dioxus, not Flutter | 5, 6 |
| 3 | Reactivity / state | (a) re-run + diff a light tree (React/Flutter, Dioxus VDOM, Xilem view tree); (b) fine-grained signals (Leptos, Floem, Slint bindings); (c) Elm messages (Iced); (d) immediate mode (egui); (e) owned entities + per-frame element rebuild (GPUI) | (a) needs keys/identity rules and a diff; Xilem uses static types to make diffing cheap. (b) no diff for updates, but components run once and structure changes need special handling; needs an arena or `Rc` for signal handles. (c) one `update(msg)` function; simplest for borrowing, more boilerplate. (d) no retained tree; layout and a11y harder. (e) app owns state in GPUI-managed handles | 4, 8, 9 |
| 4 | Retained vs immediate | retained tree; immediate; hybrid | Retained allows Flutter-style dirty flags and an AccessKit tree; immediate pays layout cost (see egui README) | 5, 7 |
| 5 | Layout | (a) own Flutter box constraints; (b) Taffy (CSS Flex/Grid/Block); (c) (a) with Taffy for some containers | (a) small core, single pass, matches the Flutter reference; every container written by you. (b) CSS semantics and a mature implementation; style structs are CSS-shaped. (c) Taffy's low-level trait API is meant for frameworks with their own tree | M3 |
| 6 | Text | (a) Parley (+Fontique, HarfRust, Skrifa, ICU4X); (b) cosmic-text; (c) Skia paragraph; (d) own | (a) Linebender stack, used by Xilem, Blitz, Slint, Floem. (b) used by Iced and GPUI's wgpu backend. (c) only if Skia is the renderer. (d) means shaping, bidi, fallback, line breaking yourself | M4 |
| 7 | Accessibility | AccessKit; none for now | AccessKit needs a node per accessible element with a stable id; adding ids later to a tree that lacks them is a rewrite | M2 design |
| 8 | API surface | builder functions; proc-macro DSL; external language | Builders work with rustfmt and IDEs and need no parser; Xilem chose "little to no reliance on macros and DSLs". Macros (`rsx!`, `view!`) give JSX-like syntax and hot-reload hooks but cost a parser and worse error messages. A separate language (Slint) adds a compiler | 9 |
| 9 | Component library | crate dependency; shadcn-style copy-in | Copy-in needs a stable, small primitives API and a CLI that writes `.rs` files into the user's crate | after M6 |

### 2.2 Notes per decision

**1. Platforms.** winit has backend crates for Android, AppKit (macOS), UIKit (iOS), Wayland, X11, Win32, Web and Orbital ([winit repo root](https://github.com/rust-windowing/winit)); its README pins `0.31.0-beta.4` while the latest tagged release is `v0.30.13` (2026-03-02). Desktop-first is the cheapest start; the cost of adding web later depends on decision 2.

**2. Rendering.** Options seen in the survey:
- Vello: the repository is now split into `vello_cpu` ("the most mature choice when predictable CPU rendering ... is required"), `vello_gpu` (CPU path preprocessing, GPU rasterization, WebGL2 and wgpu backends, "intended to become the primary renderer for production GPU use cases"), and the original compute renderer moved under `research/` ([Vello README](https://github.com/linebender/vello)). Latest release `v0.11.0` (2026-10-02).
- Skia through `skia-safe`: Freya depends on a fork, `freya-skia-safe` with the `textlayout` feature ([Freya Cargo.toml](https://github.com/marc2332/freya/blob/main/Cargo.toml)); Slint has a Skia renderer ([Slint README](https://github.com/slint-ui/slint)).
- Backend abstraction layers exist: Dioxus's `anyrender` with Vello, Vello CPU, Vello hybrid and Skia implementations ([Dioxus Cargo.toml](https://github.com/DioxusLabs/dioxus/blob/main/Cargo.toml)); Masonry's `imaging` crate with `imaging_vello`, `imaging_vello_hybrid`, `imaging_skia` features ([masonry/Cargo.toml](https://github.com/linebender/xilem/blob/main/masonry/Cargo.toml)). Putting your own thin draw-list trait between the framework and the renderer keeps this decision reversible.

**3. Reactivity, with the Rust-specific constraints.** Raph Levien's Xilem post names the problem: common UI architectures "rely on shared mutable state and that is not idiomatic Rust", and callbacks require "shared mutable access to that state, which is clunky at best in Rust (it requires interior mutability)" ([Xilem: an architecture for UI in Rust, 2022-05-07](https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html)). Event handlers stored in a tree generally have to be `'static` closures, so they cannot hold plain references into state. The surveyed projects answer this in four ways:
- **Pass `&mut AppState` in.** Xilem calls `app_logic(&mut AppData)` each cycle to produce a view tree, diffs it against the previous one, and updates the Masonry widget tree; events reach handlers with a mutable borrow of app state, routed by id paths ([xilem/ARCHITECTURE.md](https://github.com/linebender/xilem/blob/main/xilem/ARCHITECTURE.md); Levien post above).
- **Copy handles into an arena.** Dioxus's `generational-box` "allows any static type to implement `Copy`", with an `Owner` that drops values when the scope ends; `dioxus-signals` builds on it ([generational-box README](https://github.com/DioxusLabs/dioxus/tree/main/packages/generational-box)). Leptos's `reactive_graph` has signals, computations (memos) and effects, with automatic dynamic dependency tracking, based on the Reactively algorithm ([reactive_graph/src/lib.rs](https://github.com/leptos-rs/leptos/blob/main/reactive_graph/src/lib.rs)).
- **Messages.** Iced follows the Elm Architecture: views produce messages, one `update` mutates state ([Iced README](https://github.com/iced-rs/iced)).
- **Framework-owned entities.** GPUI stores state in `Entity` handles "owned by GPUI and ... only accessible through an owned smart pointer similar to an `Rc`"; a view is an entity implementing `Render`, called each frame to build elements ([GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md)).

A React/Flutter-shaped design (components re-run, light tree diffed, persistent element tree) is closest to Xilem and Dioxus. Signals avoid the diff but change the component model (components run once). This choice, more than any other, is what the project is about, so it should be decided by prototyping the counter and a list-with-keys example in two candidate models before committing. That is a recommendation, not a finding.

**4. Retained or immediate.** egui's README: "The main disadvantage of immediate mode is it makes layout more difficult", worked around by storing sizes and using them next frame, which can flicker on the first frame ([egui README](https://github.com/emilk/egui)). AccessKit says its design "is suitable for immediate-mode GUI toolkits" too, as long as they can provide stable ids ([AccessKit README](https://github.com/AccessKit/accesskit)). With Flutter as a reference, retained is the consistent choice.

**5. Layout.** Taffy implements CSS Block, Flexbox and Grid and offers a high-level `TaffyTree` and a low-level trait API (`LayoutPartialTree`, `compute_flexbox_layout`, `compute_grid_layout`) recommended "for users wanting to embed Taffy as part of ... a UI framework that already has it's own node/widget tree representation" ([taffy/src/lib.rs](https://github.com/DioxusLabs/taffy/blob/main/src/lib.rs)). Users per its README: Servo, Blitz, Slint, GPUI. Constraint-style layouts written in-house: Iced's `Limits` ("A set of size constraints for layouting") with a flex module carrying Druid's license ([iced core/src/layout](https://github.com/iced-rs/iced/tree/master/core/src/layout)); Masonry's layout pass where containers call `compute_size`, `run_layout`, `place_child` ([masonry/ARCHITECTURE.md](https://github.com/linebender/xilem/blob/main/masonry/ARCHITECTURE.md)); Freya's own `torin` crate. Text must plug into layout either way (Taffy has a measure-function hook; Flutter passes constraints to a text RenderObject).

**6. Text.** Levien's post lists the stages: paragraph segmentation, style runs, BiDi, itemization with font fallback ("the trickiest and least well specified part"), script runs, shaping, line breaking ([Text layout is a loose hierarchy of segmentation, 2020-10-26](https://raphlinus.github.io/text/2020/10/26/text-layout.html)). Reusable stacks:
- Parley: rich text layout over Fontique (enumeration, fallback), HarfRust (Rust port of HarfBuzz), Skrifa (font reading) and ICU4X ([Parley README](https://github.com/linebender/parley)). `v0.12.0`, 2026-10-09.
- cosmic-text: shaping by HarfRust, rendering by swash, bidi, font fallback; "Linux, macOS, and Windows are supported with the full feature set" ([cosmic-text README](https://github.com/pop-os/cosmic-text)). `0.19.0`, 2026-04-22.
- Skia paragraph (Flutter, Freya).
Text input with IME is a separate, later piece of work on top of any of these.

**7. Accessibility.** AccessKit provides a tree schema and platform adapters for Windows (UI Automation), macOS, Unix (AT-SPI), Android and iOS ([AccessKit README](https://github.com/AccessKit/accesskit)). It is a dependency of Xilem/Masonry, egui, Freya, Slint, GPUI and Blitz (manifests linked in section 4).

**8. API surface.** Xilem's architecture document states apps "are written with idiomatic Rust code, with little to no reliance on macros and DSLs" ([ARCHITECTURE.md](https://github.com/linebender/xilem/blob/main/ARCHITECTURE.md)). Leptos supports both: "the `view` macro above expands to this builder syntax" ([Leptos README](https://github.com/leptos-rs/leptos)). Dioxus uses `rsx!` ([Dioxus README](https://github.com/DioxusLabs/dioxus)); Slint uses a separate `.slint` language compiled to Rust/C++ ([Slint README](https://github.com/slint-ui/slint)); GPUI uses builder methods "with a tailwind-style API". Starting with builders and adding a macro that expands to them keeps the macro optional.

**9. Component library (shadcn model).** shadcn/ui says: "This is not a component library. It is how you build your component library", hands over "the actual component code", and "defines a schema for components and a CLI to distribute them" ([shadcn/ui docs](https://ui.shadcn.com/docs)). In source, the CLI's `add` command ("add a component to your project", with `--overwrite`) is [`packages/shadcn/src/commands/add.ts`](https://github.com/shadcn-ui/ui/blob/main/packages/shadcn/src/commands/add.ts); a registry component such as [`button.tsx`](https://github.com/shadcn-ui/ui/blob/main/apps/v4/registry/new-york-v4/ui/button.tsx) is styled code layered on a behaviour primitive (`Slot` from `radix-ui`), and the registry has `radix`, `base` and `aria` bases ([`apps/v4/registry/bases`](https://github.com/shadcn-ui/ui/tree/main/apps/v4/registry/bases)). The Rust equivalent would be: (1) a framework crate exposing unstyled behaviour primitives (focus, press, popover positioning) and a theme/token type; (2) a registry of `.rs` component files; (3) a `cargo`-installed CLI that copies a component into the user's `src/` and adds its crate dependencies to `Cargo.toml`. Because copied code calls framework APIs directly, every breaking change in those APIs breaks user-owned copies, so this belongs after the primitives settle. No surveyed Rust framework was checked for an existing copy-in registry; that was not verified.

### 2.3 Blocking order

```
1 platforms ──► 2 who draws ──► 5 layout, 6 text
                    │
3 reactivity ──► 4 retained/immediate ──► 7 a11y ids (in M2 design)
       │
       └──► 8 API surface ──► 9 component library
```

Decisions 1 and 3 are independent of each other and can be made in parallel. 2 depends on 1. 5 and 6 depend on 2 (and 6 partly on the renderer: Skia brings text; Vello pairs with Parley).

## 3. Build order and reasons

| # | Step | Why here | Reuse / build | Decision it needs |
|---|---|---|---|---|
| M0 | Record decisions 1 to 3 as ADRs (`docs/adr/` per this repo's `CLAUDE.md`) | Everything below depends on them | write | none |
| M1 | winit window, wgpu surface, Vello (or Skia) scene with one rounded rect; resize and redraw | Proves the platform and renderer pipeline before any framework code; every surveyed native framework starts from winit + a renderer | reuse | 1, 2 |
| M2 | Retained node tree (arena with stable ids), paint pass that walks it, a thin draw-list trait in front of the renderer, and a headless test driver | Ids are needed later by diffing (keys), hit testing and AccessKit. A test driver mirrors React's test host config and `masonry_testing` ([ARCHITECTURE.md](https://github.com/linebender/xilem/blob/main/ARCHITECTURE.md)) | build | 4, 7 |
| M3 | Layout pass: `BoxConstraints`, `layout(constraints) -> size`, parent positions child; `Row`, `Column`, `Padding`, `SizedBox`, `Center` | Text and input both need sizes and positions | build (constraints) or reuse Taffy | 5 |
| M4 | Text label: Parley layout as a leaf that answers to constraints, glyphs drawn by Vello | Smallest real use of the text stack; forces the measure-under-constraints interface early | reuse Parley | 6 |
| M5 | Pointer events from winit, hit testing against laid-out rects, `Button` with an `on_click` | Needed for any state change driven by the user | build | M2, M3 |
| M6 | State: counter app. Click updates state, the framework rebuilds what depends on it, minimal tree update, relayout/repaint only dirty parts. Then a keyed list example | This is the stated core; doing it after M5 means it is tested against real input and layout | build | 3 |
| M7 | AccessKit tree for the counter (button role, label text, action) | Confirms node ids and roles are sufficient before many widgets exist | reuse AccessKit, build mapping | 7 |
| M8+ | Focus and keyboard; text input with IME; scroll and clipping; theming; animation; then the component library and CLI | Each depends on M2 to M7 being stable | mixed | 8, 9 |

Judgement calls in this order, stated plainly:
- **M6 before M1 is a valid alternative.** The state/diff core can be built first against a headless host (React's reconciler/host split; Dioxus core emits `Mutations` for "SomeRenderer" in its [core README](https://github.com/DioxusLabs/dioxus/tree/main/packages/core)). That tests the project's main idea first. The order above puts pixels first because layout and text constraints tend to change the tree design, but this is opinion, not measured.
- **Own constraints layout versus Taffy in M3.** Constraints match the Flutter reference and are small; Taffy gives CSS Flex/Grid at once. Both are used by maintained projects.
- **Vello versus Skia in M1.** Vello keeps the build pure Rust and pairs with Parley; Skia is more mature and bundles text. The thin draw-list trait in M2 keeps this swappable.
- **Accessibility at M7 rather than later.** Placing it right after the counter is a choice to catch id design problems early.

## 4. Rust framework survey

Release and push dates are from the GitHub API on 2026-10-28. "Main" means the default branch, which can differ from the latest release.

| Project | Latest release / last push | State model | Rendering | Layout | Text | Windowing | Platforms | A11y |
|---|---|---|---|---|---|---|---|---|
| [Dioxus](https://github.com/DioxusLabs/dioxus) | v0.7.10 (2026-07-30) / 2026-10-27 | Virtual DOM in `dioxus-core`; signals on `generational-box`; `rsx!` | DOM via web-sys (web); webview via wry/tao (desktop, mobile); experimental native via Blitz + `anyrender` (Vello, Skia) | Browser CSS; Blitz uses Stylo + Taffy | Browser; Blitz uses Parley | tao (webview), winit (native) | Web, macOS, Linux, Windows, iOS, Android (README) | Blitz uses AccessKit |
| [Leptos](https://github.com/leptos-rs/leptos) | v0.9.0-rc0 (2026-10-15) / 2026-10-28 | Fine-grained signals (`reactive_graph`); `view!` or builder | DOM | Browser | Browser | n/a | Web, SSR + hydration | Browser |
| [Xilem / Masonry](https://github.com/linebender/xilem) | v0.4.0 (2025-10-29) / 2026-10-27 | Xilem: view tree from `&mut AppData`, diffed into Masonry's retained widget tree; little macro use | Vello + wgpu by default; `imaging` features for vello_hybrid and Skia | Own (Masonry layout pass) | Parley + Fontique | winit (`masonry_winit`) | Desktop; web via `xilem_web` (DOM) | AccessKit |
| [Iced](https://github.com/iced-rs/iced) | 0.14.0 (2025-12-07) / 2026-10-21 | Elm Architecture | wgpu (`iced_wgpu`) or tiny-skia (`iced_tiny_skia`) | Own (`Limits` constraints, flex from Druid) | cosmic-text (pinned git fork) | winit (pinned git fork) | Windows, macOS, Linux, Web (README) | not verified |
| [Slint](https://github.com/slint-ui/slint) | v1.18.1 (2026-09-21) / 2026-10-28 (main at 1.19.0) | `.slint` language compiled to Rust/C++/JS/Python; property binding engine with dependency tracking | femtovg (OpenGL ES 2.0), Skia, software; internal anyrender renderer crate | Own; `taffy` (flexbox) in core | Parley by default (`shared-parley`), swash, rustybuzz | winit backend, Qt, linuxkms, android-activity | Embedded, desktop, mobile, web (README) | AccessKit |
| [egui](https://github.com/emilk/egui) | 0.37.0 (2026-10-15) / 2026-10-26 | Immediate mode | epaint tessellates to textured triangles; `egui-wgpu`, glow | Own, single pass with next-frame sizes | epaint with HarfRust + Skrifa (+ vello_cpu dep) | winit via eframe | Web, Linux, Mac, Windows, Android (eframe) | AccessKit |
| [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) | no own releases (Zed v1.25.3, 2026-10-23) / 2026-10-28 | "Hybrid immediate and retained"; state in framework-owned `Entity`; views re-render elements each frame | Metal (macOS), Direct3D 11 (Windows), wgpu (`gpui_wgpu`, used by Linux and web crates) | Taffy | font-kit/CoreText (macOS), DirectWrite (Windows), cosmic-text (wgpu backend) | Own platform crates (`gpui_macos`, `gpui_windows`, `gpui_linux`, `gpui_web`, `gpui_ios`) | macOS, Linux, Windows; web and iOS crates exist | AccessKit |
| [Freya](https://github.com/marc2332/freya) | v0.4.3 (2026-08-30) / 2026-10-27 (main at 0.5.0-rc.13) | Own component model (`impl Component`, `use_state`); main no longer depends on Dioxus core (only `dioxus-devtools` for hot reload) | Skia (`freya-skia-safe`, GL / Vulkan / Metal) | Own (`torin`) | Skia textlayout | winit + glutin | Desktop; `freya-android`, `freya-web` crates exist | AccessKit |
| [Floem](https://github.com/lapce/floem) | v0.2.0 (2024-11-15) / 2026-09-24 | Fine-grained signals inspired by `leptos_reactive` | wgpu via vger or Vello; Skia via AnyRender; tiny-skia CPU fallback | Taffy (Flexbox, Grid) | Parley 0.7 | winit (Lapce fork) | Windows, macOS, Linux | not verified |

Supporting crates:

| Crate | Role | Latest release (date) |
|---|---|---|
| [winit](https://github.com/rust-windowing/winit) | windowing and input | v0.30.13 (2026-03-02); README pins 0.31.0-beta.4 |
| [wgpu](https://github.com/gfx-rs/wgpu) | GPU API over Vulkan/Metal/DX12/GL/WebGPU | v30.0.1 (2026-08-22) |
| [Vello](https://github.com/linebender/vello) | 2D renderer (CPU, GPU, compute research) | v0.11.0 (2026-10-02) |
| [Parley](https://github.com/linebender/parley) | rich text layout | v0.12.0 (2026-10-09) |
| [cosmic-text](https://github.com/pop-os/cosmic-text) | text shaping and layout | 0.19.0 (2026-04-22) |
| [Taffy](https://github.com/DioxusLabs/taffy) | CSS Block/Flex/Grid layout | v0.15.1 (2026-10-25) |
| [AccessKit](https://github.com/AccessKit/accesskit) | accessibility tree and adapters | `accesskit_winit-v0.34.2` (2026-10-25) |
| [kurbo](https://github.com/linebender/kurbo) | 2D geometry and curves | v0.13.1 (2026-05-13) |
| [tiny-skia](https://github.com/linebender/tiny-skia) | CPU 2D rasterizer | no GitHub releases; last push 2026-08-20 |
| [rust-skia](https://github.com/rust-skia/rust-skia) | Skia bindings | no GitHub releases; last push 2026-10-26 |

Maintenance notes: all surveyed repositories had pushes within the last five weeks of 2026-10-28 and none is archived. The old standalone `linebender/masonry` repository is archived (last push 2024-05-29); Masonry now lives in the Xilem repository. Floem's last GitHub release is from 2024-11-15 although the repository is still updated. Flutter's GitHub "latest release" entry is from 2024 (`3.19.0-0.1.pre`) and does not reflect its actual release cadence; Flutter's releases were not checked further.

## 5. Not verified

- Iced and Floem accessibility support (no AccessKit dependency seen in their root manifests; not checked further).
- Maturity of GPUI's `gpui_web` and `gpui_ios`, Freya's `freya-android` and `freya-web`, and Masonry's Android driver (named "Masonry Android View" in the Xilem architecture diagram; its repository was not found under `linebender/masonry_android_view`).
- Why Xilem's main branch pins `vello 0.8` and `wgpu 28` while Vello 0.11 and wgpu 30 are released.
- Exact role of `vello_cpu` inside egui's `epaint` (only the dependency was seen).
- Whether any Rust UI project already offers a shadcn-style copy-in component registry.
- Slint's mobile and web support level beyond the README statement.
- Performance claims of any framework; nothing was measured.

## Sources

React (pinned `dc9aed3`):
- https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-reconciler/README.md
- https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-reconciler/src/ReactFiberWorkLoop.js
- https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-reconciler/src/ReactFiberHooks.js
- https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-reconciler/src/ReactChildFiber.js
- https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-reconciler/src/ReactFiberCommitWork.js
- https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-reconciler/src/ReactFiberLane.js
- https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/scheduler/src/forks/Scheduler.js
- https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/scheduler/src/SchedulerFeatureFlags.js
- https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-dom-bindings/src/client/ReactFiberConfigDOM.js
- https://github.com/react/react/blob/dc9aed364252a34966f4f2c4a98272317fdddb60/packages/react-test-renderer/src/ReactFiberConfigTestHost.js
- https://react.dev/learn/render-and-commit

Flutter (pinned `7102630`):
- https://github.com/flutter/flutter/blob/710263041fe6f956e7a328756c63d751cd27f98a/packages/flutter/lib/src/widgets/framework.dart
- https://github.com/flutter/flutter/blob/710263041fe6f956e7a328756c63d751cd27f98a/packages/flutter/lib/src/rendering/object.dart
- https://github.com/flutter/flutter/blob/710263041fe6f956e7a328756c63d751cd27f98a/packages/flutter/lib/src/rendering/box.dart
- https://github.com/flutter/flutter/blob/710263041fe6f956e7a328756c63d751cd27f98a/docs/about/The-Engine-architecture.md
- https://github.com/flutter/flutter/tree/710263041fe6f956e7a328756c63d751cd27f98a/engine/src/flutter/txt/src/skia
- https://docs.flutter.dev/resources/architectural-overview
- https://docs.flutter.dev/ui/layout/constraints
- https://docs.flutter.dev/perf/impeller

shadcn/ui:
- https://ui.shadcn.com/docs
- https://github.com/shadcn-ui/ui/blob/main/packages/shadcn/src/commands/add.ts
- https://github.com/shadcn-ui/ui/blob/main/apps/v4/registry/new-york-v4/ui/button.tsx
- https://github.com/shadcn-ui/ui/tree/main/apps/v4/registry/bases

Rust frameworks and crates (default branch, read 2026-10-28):
- Dioxus: https://github.com/DioxusLabs/dioxus (README, Cargo.toml), https://github.com/DioxusLabs/dioxus/tree/main/packages/core, https://github.com/DioxusLabs/dioxus/tree/main/packages/generational-box
- Blitz: https://github.com/DioxusLabs/blitz
- Leptos: https://github.com/leptos-rs/leptos, https://github.com/leptos-rs/leptos/blob/main/reactive_graph/src/lib.rs
- Xilem / Masonry: https://github.com/linebender/xilem, https://github.com/linebender/xilem/blob/main/ARCHITECTURE.md, https://github.com/linebender/xilem/blob/main/xilem/ARCHITECTURE.md, https://github.com/linebender/xilem/blob/main/masonry/ARCHITECTURE.md, https://github.com/linebender/xilem/blob/main/masonry/Cargo.toml
- Iced: https://github.com/iced-rs/iced, https://github.com/iced-rs/iced/tree/master/core/src/layout
- Slint: https://github.com/slint-ui/slint, https://github.com/slint-ui/slint/blob/master/internal/core/Cargo.toml, https://github.com/slint-ui/slint/blob/master/internal/core/properties.rs, https://github.com/slint-ui/slint/blob/master/LICENSE.md
- egui: https://github.com/emilk/egui, https://github.com/emilk/egui/blob/main/crates/epaint/Cargo.toml
- GPUI: https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md, https://github.com/zed-industries/zed/blob/main/crates/gpui/Cargo.toml, https://github.com/zed-industries/zed/tree/main/crates/gpui_wgpu, https://github.com/zed-industries/zed/tree/main/crates/gpui_windows
- Freya: https://github.com/marc2332/freya, https://github.com/marc2332/freya/blob/main/Cargo.toml, https://github.com/marc2332/freya/blob/main/crates/freya-engine/Cargo.toml
- Floem: https://github.com/lapce/floem
- Taffy: https://github.com/DioxusLabs/taffy, https://github.com/DioxusLabs/taffy/blob/main/src/lib.rs
- Parley: https://github.com/linebender/parley
- Vello: https://github.com/linebender/vello
- cosmic-text: https://github.com/pop-os/cosmic-text
- AccessKit: https://github.com/AccessKit/accesskit
- winit: https://github.com/rust-windowing/winit

Author posts:
- Raph Levien, "Xilem: an architecture for UI in Rust", 2022-05-07: https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html
- Raph Levien, "Text layout is a loose hierarchy of segmentation", 2020-10-26: https://raphlinus.github.io/text/2020/10/26/text-layout.html
