# mountain

A Rust UI framework with built-in state management that draws its own UI on every platform.

## Language

**Core**:
The part of the framework that holds the state model, the widget tree, layout and events, and depends on no rendering, text or windowing crate.
_Avoid_: engine, runtime

**Backend**:
An implementation of one of the core's traits on top of an outside crate or platform, such as a renderer built on Vello or a platform built on winit.
_Avoid_: driver, adapter, host

**Constraints**:
The minimum and maximum width and height a parent gives a child during layout; the child picks its size within them.
_Avoid_: layout props, size hints

**Semantics tree**:
The description of the UI's meaning (roles, labels, actions) that assistive technology reads, with a stable id per node.
_Avoid_: accessibility tree, a11y tree
