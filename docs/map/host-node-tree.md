# The tree of Host nodes

How the Core keeps its Host nodes and turns them into a frame. Decisions: ADR-0006 (Node ids), ADR-0007 (Display list caching); the M2 spec is #12. Code: `Tree`, `HostNode`, `DisplayList` and `ColoredBox` in `crates/mountain-core/src/tree.rs`, and `Core::draw`, which replays the lists.

## Shape

- `Tree` holds every Host node in one map from Node id to entry. Each entry has the boxed Host node, its bounds and its children's Node ids in drawing order. A map rather than a `Vec` indexed by id, because Node ids are never reused (ADR-0006), so ids grow without bound and cannot double as indices.
- Node ids come from one counter for the whole process, not one per `Tree`. ADR-0006 says no two Host nodes ever share an id, and a `Tree` can be made or swapped in through `Core::tree_mut`: with a counter per tree, a Node id kept from a replaced tree would name a node in the new one and act on it without a panic.
- The root exists from the start, at the window's origin with zero size, and paints nothing. It is a private type so an app cannot replace it.
- Bounds are the node's position **relative to its parent** and its size, in logical pixels. In M2 the app sets them; M3 layout takes this over (ADR-0004: the parent places the child).

## Painting a frame

- The walk starts at the root and goes parent first, then children in order, so a parent is under its children and a later sibling over an earlier one.
- A Host node paints at its size, in its own coordinates, into a Display list. The Core adds the accumulated parent positions to each command, then converts to physical pixels at the current scale factor, then calls the `Renderer`. The `Renderer` trait did not change for the tree.
- The background, a Core setting rather than a Host node, is drawn before the tree.
- Nothing is clipped: a child outside its parent's box is still drawn.

## Traps

- Nothing stops `set_bounds` on the root yet, so it can move the whole tree away from the window's origin.
- `DisplayList`'s commands are private to the Core: a Host node can add to a list but nobody outside the Core reads one. Tests observe painting through the recording renderer instead.
