# Node ids are never reused

A Host node's Node id is a non-zero `u64` taken from a counter that only goes up, so no two Host nodes ever share one, even after the first is removed; using a removed Node id is a caller's bug and panics. This answers the open line in ADR-0005, whose prototype reused Instance ids through a free list: ADR-0003 asks for a stable id per node, and AccessKit, which the Semantics tree is meant to feed, calls its `NodeId` "the stable identity of a `Node`" and applies a `TreeUpdate` by overwriting whatever node has the same id (`accesskit/src/lib.rs` in AccessKit/accesskit, read 2026-10-29). A reused id could be read there as an update of the removed node; that reading is an inference, since the AccessKit docs state no rule on reuse either way. It also keeps ADR-0005's property that a broken rule panics instead of leaving a stale screen. The cost is a map lookup instead of an index into a `Vec`. This was the maintainer's call (M2 design session, 2026-10-29), so it is theirs to reverse.

## Considered options

Read in each project's source on 2026-10-29 (details in `docs/research/m2-tree-ids-and-paint.md`):

- **A counter that only goes up**: Masonry (`WidgetId`), Freya (`NodeId`), Iced (`widget::Id::Unique`). Chosen.
- **A generational slot map** (the `slotmap` crate): Floem (`ViewId`), GPUI (`EntityId`). A slot is reused with a new version, so a stale key fails its lookup; rejected because handing the id to AccessKit would need a separate rule for turning a key into a `u64`.
- **A slab that reuses freed slots with no check**: Dioxus (`ElementId`, whose docs say it "may be reused for a new element"). Rejected because a stale id would silently reach another node.
