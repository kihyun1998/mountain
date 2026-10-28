# The state model is re-run and diff

A Component re-runs when its own state changes or its props differ, and its output is diffed against the kept Instance tree, as in React and Flutter; children read parent state through value props and write through a comparable, write-only setter. This was the maintainer's call (#5, 2026-10-28), made between two headless prototypes that ran the same scenarios, so it is theirs to reverse. In the prototype, breaking a rule (hook order, a setter used during render, duplicate keys) panicked rather than leaving a stale screen; the Core is to keep that property.

## What the call was made on

Counts are reruns / nodes changed / elements diffed, from the prototypes' tests (`prototypes/rerun-diff` at 0e5db9c, `prototypes/signals` at a6fd2ee; their NOTES.md define each count, and the units of the last two differ between models).

| Case | Re-run and diff | Fine-grained signals |
|---|---|---|
| Counter click | 1 / 1 / 5 | 1 / 1 / 0 |
| Change one of 100 items through its own state | 1 / 1 / 5 | 1 / 1 / 0 |
| Change one of 100 items through list state | 2 / 1 / 112, correct | 1 / 0 / 100, screen not updated |
| The same, with a signal per row | — | 1 / 1 / 0 |
| Insert, remove or reverse in a 100-item list | same reruns and nodes changed in both | |

The maintainer was also shown an adversarial read of the options: re-run and diff maps onto a Flutter-style Widget/Element/RenderObject split and fails loudly; signals cost least per update but need signals inside list data, have two traps that compile and leave a stale screen, and leak signals created in event handlers in the prototype; and the state model lives in the Core's public API, so ADR-0002 does not make this choice cheap to reverse.

## Considered options

- **Fine-grained signals** (Leptos, Floem, Slint): rejected for the reasons above.
- **Re-run and diff with subscribe-on-read** (Dioxus): not prototyped and not measured; reaching it from this model would change the runtime's dirty marking rather than app code, which is an unmeasured inference.

## Not tested, and not settled by this call

High-frequency updates such as animation, deep nesting without equal props, derived values, context, effects and cleanup, async, and many windows were exercised by neither prototype. Event handlers are reallocated on every render, which no count includes. The prototype's names and signatures (`use_state`, `Setter`, `PartialEq` props) show the model; they are not the Core's API. Instance ids are reused through a free list in the prototype, which the stable ids of ADR-0003 may not allow.
