# The tree of Host nodes

How the Core keeps its Host nodes and turns them into a frame. Decisions: ADR-0006 (Node ids), ADR-0007 (Display list caching); the M2 spec is #12. Code: `Tree`, `HostNode`, `DisplayList` and `ColoredBox` in `crates/mountain-core/src/tree.rs`, and `Core::draw`, which replays the lists.

## Shape

- `Tree` holds every Host node in one map from Node id to entry. Each entry has the boxed Host node, its bounds and its children's Node ids in drawing order. A map rather than a `Vec` indexed by id, because Node ids are never reused (ADR-0006), so ids grow without bound and cannot double as indices.
- Node ids come from one counter for the whole process, not one per `Tree`. ADR-0006 says no two Host nodes ever share an id, and a `Tree` can be made or swapped in through `Core::tree_mut`: with a counter per tree, a Node id kept from a replaced tree would name a node in the new one and act on it without a panic.
- The root exists from the start, at the window's origin with zero size, and paints nothing. It is a private type so an app cannot replace it.
- Each entry also keeps its parent's Node id, so removing or moving a node edits one parent's child list instead of searching the tree.
- Bounds are the node's position **relative to its parent** and its size, in logical pixels. In M2 the app sets them; M3 layout takes this over (ADR-0004: the parent places the child).

## Changing the tree

- `update` is the only way to reach a Host node's values. It downcasts through `Any`, which `HostNode` has as a supertrait (as Masonry's `Widget` does), and panics when the node is not the type asked for. It drops the node's kept Display list before handing the node out, so a `change` that panics part-way still leaves the node to paint again.
- `remove` takes the whole subtree; every Node id in it stops being live and is never handed out again (ADR-0006).
- `move_to` reorders a node among its siblings, which changes only drawing order.
- Any operation on a Node id that is not live panics with the id in the message, so a stale id fails at once instead of leaving a stale screen (ADR-0005). `contains` answers the question without panicking.
- The root cannot be removed, moved among siblings or given bounds: each panics. That keeps it at the window's origin, which the frame walk starts from.

## Painting a frame

- Each entry keeps the Display list from its last paint (ADR-0007). A frame first paints every Host node that has none kept, then walks the tree replaying the kept lists. A node has none kept when it is new, after `update`, and after `set_bounds` changes its width or height.
- Only the size reaches `paint`, so a change of position, an order change among siblings and a new scale factor leave every list as it was: the lists are in the node's own coordinates and in logical pixels, and the walk adds the offsets and the scale. Masonry likewise repaints a widget whose size changed in layout and not one that only moved (`masonry_core/src/passes/layout.rs`).
- On a scale change this differs from Masonry, which marks every widget for paint (`request_render_all` on `WindowEvent::Rescale`, `masonry_core/src/app/render_root.rs`), and from GPUI, whose `set_scale_factor` refreshes the window so cached views are not reused. Flutter is the matching case: a new device pixel ratio replaces the root transform layer (`rendering/view.dart`). It holds here because `paint` is given no scale; a Host node whose drawing depends on the scale, such as one snapping to physical pixels, would break it.
- Stale nodes paint in the map's order, not tree order, before the walk. Nothing depends on that order while `paint` reads only its own values and size.
- The kept list lives in the entry, so `remove` drops it with the node.
- Not measured: the cost of joining many small lists each frame, and the memory a list per node takes (ADR-0007, Consequences).
- The walk starts at the root and goes parent first, then children in order, so a parent is under its children and a later sibling over an earlier one.
- A Host node paints at its size, in its own coordinates, into a Display list. The Core adds the accumulated parent positions to each command, then converts to physical pixels at the current scale factor, then calls the `Renderer`. The `Renderer` trait did not change for the tree.
- The background, a Core setting rather than a Host node, is drawn before the tree.
- Nothing is clipped: a child outside its parent's box is still drawn.

## Traps

- A Host node's `paint` must depend only on its own values and the size it is given. A node that reads anything else, such as a clock or shared state, is not painted again when that changes, because nothing marks it.
- `DisplayList`'s commands are private to the Core: a Host node can add to a list but nobody outside the Core reads one. Tests observe painting through the recording renderer instead.
