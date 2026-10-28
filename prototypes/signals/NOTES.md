# Fine-grained signals prototype: results

Prototype for #3. Throwaway: deleted once the state-model ADR (#5) lands. It runs
the scenario script in `prototypes/rerun-diff/NOTES.md` and counts the same
metrics, defined the same way.

## Model

A component is a plain function that runs once and returns a view. `signal(v)`
returns a `Signal<T>` handle that is `Copy`, owned by the scope that created it.

- Dynamic parts of a view are closures: `text_fn(move || …)` and
  `attr_fn(name, move || …)`. Each becomes an effect. Reading a signal inside an
  effect subscribes the effect; writing the signal marks its subscribers dirty.
  After the event handler returns, dirty effects re-run in creation order, so
  updates within one event are batched, as in the re-run-and-diff prototype.
  An effect re-subscribes on every run.
- A read outside an effect (in the component function itself) subscribes
  nothing: the value is used once and never updates.
- Lists use `el(..).each(items, key, row)`. `row` builds a row once, in a scope
  of its own, untracked, as Leptos runs `For`'s children. When `items()`
  changes, rows are matched by key: new keys get a new row, gone keys have their
  scope disposed (its signals, effects and nodes go with it), and a kept row is
  **not rebuilt even if its item changed**. That is how Leptos's `For` behaves:
  `tachys/src/view/keyed.rs` diffs only the keys and calls the view function
  only for added items. Moves are counted under React's `placeChild` rule so
  both prototypes count them alike; tachys uses its own move algorithm.
- Two rows with the same key panic, before any row is touched. An element with
  `each` cannot also have static children; that panics too. A signal used after
  its scope was disposed, or outside an app, panics. A reader or an effect that
  panics leaves the signal and the tracking state intact.
- Disposing a row's scope removes its nodes first, then its child scopes, so a
  row holding a nested list goes away cleanly.
- `update` and `set` mark subscribers dirty even when the value did not change,
  as the re-run-and-diff prototype does. An effect that writes a signal it reads
  re-runs itself without end; so does a component in the re-run-and-diff
  prototype that writes its own state while rendering.
- Out of scope, here and in the spec: memos and derived signals, async,
  context. Leptos has all three.

## Metrics, as counted

- **nodes changed**: the same definition as `prototypes/rerun-diff/NOTES.md`
  (created, removed, attribute or text changed, plus moved rows).
- **reruns**: effect runs caused by a write during the event, plus rows built
  for new keys. An effect's first run when it is created is not counted.
- **elements diffed**: every key of the new item list, each time a list's items
  change. Nothing else is diffed in this model.

The last two have different units in the two prototypes, so equal numbers do
not mean equal work:

- A re-run here is one small closure (a text or attribute effect); a re-run in
  the re-run-and-diff prototype is a whole component `render`.
- Elements diffed here are list keys; there they are element-to-instance
  comparisons, including every element inside each re-run component.
- Neither counts the re-subscription an effect does on every run here, nor the
  event handlers replaced on every render there.

Cross-check on a four-item list (same buttons, printed by a throwaway probe):
insert at front or middle 2 / 5 / 5, remove 1 / 5 / 4, reverse 1 / 3 / 4. The
re-run-and-diff prototype gives the same reruns and nodes changed, with 11
elements diffed for each.

## Update cost (from `cargo test -- --nocapture`)

| Case | reruns | nodes changed | elements diffed |
|---|---|---|---|
| Counter, one click | 1 | 1 | 0 |
| 100-item list, rename item 50 (its own label signal) | 1 | 1 | 0 |
| 100-item list, type into item 50 (item state) | 1 | 1 | 0 |
| 100-item list, replace item 50 in list state (stale screen) | 1 | 0 | 100 |

**The rename row is not the same case as the re-run-and-diff prototype's.**
There, rename writes the list state (2 / 1 / 112). Here, rename writes a signal
the row owns, the way Leptos's `For` documentation does it, which makes it the
same kind of change as typing into the row. The like-for-like comparison for
renaming through list state is the last row: writing the list re-runs the list
effect and compares 100 keys, and the screen does not change, because a kept
row is never rebuilt (`replacing_an_item_under_the_same_key_leaves_its_row_stale`).
A correct rename through list state does not exist in this model; the data has
to hold a signal per row.

## App-author friction (in `tests/`)

- `Rc<RefCell<…>>`: none.
- Lifetime annotations: none written.
- Rebinding before a `move` closure: none. A row function owns its item, so a
  closure captures `row.label` alone (Rust 2021 disjoint capture).
- `clone()`: one, `row.key.clone()` in the key function.
- `#[derive(Clone)]` on the row type, which `items.get()` needs.
- Hidden copies: `items.get()` clones the `Vec<Row>` each time the list effect
  runs; `row.label.get()` and `draft.get()` clone a `String` each time their
  effect runs.
- Nested state: to change a row in place, the row's data has to be a signal
  inside the list's data (`Row { key, label: Signal<String> }`).
- Silent traps, both compile and show a stale screen:
  - replacing an item under the same key (above);
  - reading a signal outside a closure, such as `text(count.get().to_string())`
    (`a_signal_read_outside_an_effect_never_updates_the_screen`).
- Ownership: signals created in an event handler, such as a new row's label,
  belong to the scope that was current then (the app's root scope here), not to
  the row, so removing the row does not free them. The same holds for the label
  signals created with the initial list.
- No hook-order rule: signals are created once, so creating one conditionally is
  fine.
