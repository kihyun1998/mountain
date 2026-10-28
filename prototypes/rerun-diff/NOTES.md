# Re-run-and-diff prototype: results

Prototype for #2. Throwaway: deleted once the state-model ADR (#5) lands.

## Model

A component is a `PartialEq` struct with `render(&self, cx) -> El`. State lives in
a per-app slot store; `cx.use_state(init)` returns a `State<T>` handle that is
`Copy`, so `'static` event handlers capture it by `move` with no `Rc`.

- Writing a state marks its owning component dirty. After the event handler
  returns, dirty components re-run, shallowest first, and each output is diffed
  against the kept instance tree. Updates within one event are batched.
- A child component whose props compare equal is not re-run (Dioxus's default;
  React needs `memo` for it). Not verified against Dioxus's source.
- Children read parent state only through **value props**. `State` is not
  `PartialEq`, so it cannot sit in props; `state.setter()` gives a write-only,
  comparable `Setter` for that. This follows React's `useState`, which returns a
  value and a setter. A `State` in a child's props no longer compiles. A `Setter`
  used while any component renders panics ("cannot update another component's
  state while rendering"), as React rejects the same in `ReactFiberWorkLoop.js`;
  without that check a child could read through `Setter::update` during render
  and show a stale value. A hand-written `PartialEq` on props can still smuggle a
  `State` in; nothing stops that.
- Hooks are identified by call order. A render that calls more or fewer hooks
  than the previous one, or a hook of another type at the same position, panics
  with "hook order changed" (React reports the same in development).
- Keyed children are matched by key; two siblings with the same key panic.
  Moves follow React's `placeChild` rule (`lastPlacedIndex`). A component whose
  output's root key changes gets a fresh instance.
- `update` and `set` mark the owner dirty even when the value did not change;
  React bails out when the new state is `Object.is`-equal. A no-op update costs
  one re-run here.
- Out of scope, here and in the spec: effects and cleanup, context, async.

## Metrics, as counted

- **reruns**: calls to a component's `render` during one event.
- **nodes changed**: output nodes (host elements and text) created, removed, or
  whose attributes or text changed, plus kept children that moved. A move is
  counted once per moved child, whether that child is a host node or a
  component. Mounting a subtree counts every host and text node in it. Replacing
  event handlers is not counted.
- **elements diffed**: calls to the reconciler that compare one new element
  with one kept instance. This cost exists only in a diffing model.

## Scenario script

The tests in `tests/` run these, and #3 has to run the same:

- Counter: shows `0`; click `inc` once → `1`; twice more → `3`.
- Keyed list `a b c`: type `alpha` into `a`, `gamma` into `c`; reverse → order
  `c b a`, drafts kept; insert at front → `new0 c b a`, `new0` empty; remove the
  first item twice → `b a`, drafts kept.
- Keyed list `a b c d`: type `alpha`, `gamma`, `delta` into `a`, `c`, `d`; insert
  at the middle (index `len / 2`) → `a b new0 c d`; remove at the middle twice →
  `a b d`, drafts kept.
- Cost: one counter click; in a 100-item list, rename item 50 through list
  state; in the same list, type into item 50's own state.

## Update cost (from `cargo test -- --nocapture`)

| Case | reruns | nodes changed | elements diffed |
|---|---|---|---|
| Counter, one click | 1 | 1 | 5 |
| 100-item list, rename item 50 (list state) | 2 | 1 | 112 |
| 100-item list, type into item 50 (item state) | 1 | 1 | 5 |

Without the props-equality skip, renaming item 50 re-runs 101 components
(measured by mutating the skip off).

A change in list-level state re-runs the list and diffs every child, so the diff
work grows with the list length even when one item changed.

## App-author friction (in `tests/`)

- `Rc<RefCell<…>>`: none.
- Lifetime annotations: none written.
- Rebinding before a `move` closure: `let items = self.items;` in `Item`. Without
  it the closure borrows `self` and fails the `'static` bound (E0521).
- `clone()`: four, all of owned `String`/`Vec` data, none of state handles:
  `self.initial.clone()` for the initial state, `key.clone()` for the child key,
  `self.key.clone()` captured by the rename handler, `self.label.clone()` for
  the text node.
- Hidden copies: `items.get()` clones the whole `Vec` on every render of the
  list, and `draft.get()` clones the `String` on every render of an item.
  `with(|v| …)` avoids a copy but cannot hand out owned values to children.
- Callbacks as props would break the props-equality skip (closures are not
  `PartialEq`); a `Setter` passed down instead keeps it.
- Rules the compiler does not check, caught at run time instead: hook order,
  duplicate keys, a `Setter` used during render, and a handle used after its
  component unmounted or outside an app. A reader that panics inside `with`
  leaves the state intact.
