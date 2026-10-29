# mountain

A Rust UI framework with built-in state management that draws its own UI on every platform.

## Language

**Core**:
The part of the framework that holds the state model, the tree of Instances and Host nodes, layout and events, and depends on no rendering, text or windowing crate.
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

## The tree

**Component**:
A piece of UI described by its props, whose render produces the Components and Host nodes under it; it is created anew on every render.
_Avoid_: widget, view

**Instance**:
The kept counterpart of a Component across renders, holding its state and its place in the tree.
_Avoid_: element, fiber

**Host node**:
A node the framework itself lays out and draws, as opposed to a Component.
_Avoid_: render object, DOM node

**Node id**:
The identity of one Host node, never given to another Host node, even after the first is removed.
_Avoid_: widget id, element id, key

**Display list**:
The drawing commands a Host node records when it paints, kept and replayed until the node changes.
_Avoid_: scene, picture, draw list

## Units

**Logical pixel**:
A unit of length that does not depend on the display; one logical pixel covers as many physical pixels as the scale factor says.
_Avoid_: point, dp, device-independent pixel

**Physical pixel**:
One pixel of the surface a frame is drawn into.
_Avoid_: device pixel, raw pixel

**Scale factor**:
How many physical pixels one logical pixel spans along each axis.
_Avoid_: DPI, density, zoom
