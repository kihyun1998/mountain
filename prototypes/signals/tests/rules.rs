use signals::*;

fn prefixed_list() -> View {
    let prefix = signal("item".to_string());
    let items = signal(vec![1, 2, 3]);
    el("div")
        .child(el("button").id("rename-prefix").on_click(move || prefix.set("row".to_string())))
        .child(el("ul").each(
            move || items.get(),
            |n| *n,
            move |n| {
                let built_with = prefix.get();
                el("li").id(format!("row-{n}")).child(text(format!("{built_with} {n}"))).into()
            },
        ))
        .into()
}

#[test]
fn a_signal_read_while_building_a_row_does_not_rebuild_the_list() {
    let mut app = App::new(prefixed_list);
    assert!(app.ids().contains(&"row-1".to_string()));
    app.click("rename-prefix");
    let stats = app.last_update();
    assert_eq!(stats.reruns, 0);
    assert_eq!(stats.elements_diffed, 0);
    assert_eq!(app.text("row-1"), "item 1");
}

fn twins() -> View {
    el("ul").each(|| vec![1, 1], |n| *n, |n| text(n.to_string())).into()
}

#[test]
#[should_panic(expected = "duplicate key")]
fn two_rows_with_the_same_key_are_rejected() {
    App::new(twins);
}

fn sum() -> View {
    let a = signal(1);
    let b = signal(2);
    el("span").id("sum").child(text_fn(move || a.with(|x| x + b.get()).to_string())).into()
}

#[test]
fn reading_one_signal_inside_another_signals_with_works() {
    let app = App::new(sum);
    assert_eq!(app.text("sum"), "3");
}

fn fragile() -> View {
    let n = signal(0);
    el("div")
        .child(el("span").id("n").child(text_fn(move || n.get().to_string())))
        .child(el("button").id("boom").on_click(move || n.with(|_| panic!("reader failed"))))
        .child(el("button").id("inc").on_click(move || n.update(|v| *v += 1)))
        .into()
}

#[test]
fn a_signal_is_usable_after_a_reader_panicked() {
    let mut app = App::new(fragile);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.click("boom")));
    assert!(caught.is_err());
    app.click("inc");
    assert_eq!(app.text("n"), "1");
}

fn static_read() -> View {
    let count = signal(0);
    el("div")
        .child(el("span").id("count").child(text(count.get().to_string())))
        .child(el("button").id("inc").on_click(move || count.update(|n| *n += 1)))
        .into()
}

#[test]
fn a_signal_read_outside_an_effect_never_updates_the_screen() {
    let mut app = App::new(static_read);
    app.click("inc");
    assert_eq!(app.text("count"), "0");
    assert_eq!(app.last_update().reruns, 0);
}

fn nested() -> View {
    let outer = signal(vec![1, 2]);
    el("div")
        .child(el("button").id("drop-first-outer").on_click(move || {
            outer.update(|v| {
                v.remove(0);
            })
        }))
        .child(el("ul").id("outer").each(
            move || outer.get(),
            |n| *n,
            |n| {
                let inner = signal(vec![10, 20]);
                el("li")
                    .child(el("button").id(format!("drop-first-inner-{n}")).on_click(move || {
                        inner.update(|v| {
                            v.remove(0);
                        })
                    }))
                    .child(el("ul").each(move || inner.get(), |m| *m, move |m| {
                        el("li").id(format!("cell-{n}-{m}")).into()
                    }))
                    .into()
            },
        ))
        .into()
}

#[test]
fn removing_a_row_that_holds_a_nested_list_removes_its_cells() {
    let mut app = App::new(nested);
    app.click("drop-first-outer");
    let ids = app.ids();
    assert!(!ids.contains(&"cell-1-10".to_string()));
    assert!(ids.contains(&"cell-2-10".to_string()));
}

#[test]
fn removing_a_row_after_removing_one_of_its_nested_rows_works() {
    let mut app = App::new(nested);
    app.click("drop-first-inner-1");
    app.click("drop-first-outer");
    assert!(app.ids().contains(&"cell-2-20".to_string()));
}

fn fragile_effect() -> View {
    let n = signal(0);
    let m = signal(0);
    el("div")
        .child(el("span").id("n").child(text_fn(move || {
            let v = n.get();
            assert!(v != 1, "effect failed");
            v.to_string()
        })))
        .child(el("button").id("inc").on_click(move || n.update(|v| *v += 1)))
        .child(el("button").id("read-m").on_click(move || {
            let _ = m.get();
        }))
        .child(el("button").id("write-m").on_click(move || m.update(|v| *v += 1)))
        .into()
}

#[test]
fn an_effect_that_panicked_does_not_capture_later_reads() {
    let mut app = App::new(fragile_effect);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.click("inc")));
    assert!(caught.is_err());
    app.click("read-m");
    app.click("write-m");
    assert_eq!(app.last_update().reruns, 0);
}

fn twins_later() -> View {
    let items = signal(vec![1, 2]);
    el("div")
        .child(el("button").id("dup").on_click(move || items.set(vec![1, 1])))
        .child(el("button").id("restore").on_click(move || items.set(vec![1, 2])))
        .child(el("ul").each(move || items.get(), |n| *n, |n| {
            let draft = signal(String::new());
            el("input")
                .id(format!("input-{n}"))
                .attr_fn("value", move || draft.get())
                .on_input(move |s| draft.set(s.to_string()))
                .into()
        }))
        .into()
}

#[test]
fn a_duplicate_key_panic_leaves_the_rows_in_place() {
    let mut app = App::new(twins_later);
    app.input("input-1", "kept");
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.click("dup")));
    assert!(caught.is_err());
    app.click("restore");
    assert_eq!(app.attr("input-1", "value").unwrap(), "kept");
}

fn each_with_child() -> View {
    el("ul").child(el("li").id("static")).each(|| vec![1], |n| *n, |n| text(n.to_string())).into()
}

#[test]
#[should_panic(expected = "each")]
fn an_element_with_each_cannot_also_have_static_children() {
    App::new(each_with_child);
}

#[derive(Clone)]
struct Labeled {
    key: u32,
    label: Signal<String>,
}

fn replaceable_list() -> View {
    let items = signal((0..100).map(|key| Labeled { key, label: signal(format!("Item {key}")) }).collect::<Vec<_>>());
    el("div")
        .child(el("button").id("replace-50").on_click(move || {
            let renamed = signal("Item 50!".to_string());
            items.update(|v| v[50] = Labeled { key: 50, label: renamed });
        }))
        .child(el("ul").each(move || items.get(), |r| r.key, |r| {
            el("li").id(format!("label-{}", r.key)).child(text_fn(move || r.label.get())).into()
        }))
        .into()
}

#[test]
fn replacing_an_item_under_the_same_key_leaves_its_row_stale() {
    let mut app = App::new(replaceable_list);
    app.click("replace-50");
    assert_eq!(app.text("label-50"), "Item 50");
    let stats = app.last_update();
    assert_eq!(stats.reruns, 1);
    assert_eq!(stats.nodes_changed, 0);
    assert_eq!(stats.elements_diffed, 100);
    println!("signals, replace item 50 in list state: {stats:?}");
}
