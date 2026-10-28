use signals::*;

#[derive(Clone)]
struct Row {
    key: String,
    label: Signal<String>,
}

fn todo_list(initial: Vec<(String, String)>) -> View {
    let items = signal(
        initial
            .into_iter()
            .map(|(key, label)| Row { key, label: signal(label) })
            .collect::<Vec<_>>(),
    );
    let next = signal(0);
    let fresh = move || {
        let n = next.get();
        next.set(n + 1);
        Row { key: format!("new{n}"), label: signal(format!("New {n}")) }
    };
    el("div")
        .child(el("button").id("reverse").on_click(move || items.update(|v| v.reverse())))
        .child(el("button").id("insert-front").on_click(move || {
            let row = fresh();
            items.update(|v| v.insert(0, row));
        }))
        .child(el("button").id("remove-first").on_click(move || {
            items.update(|v| {
                v.remove(0);
            })
        }))
        .child(el("button").id("insert-middle").on_click(move || {
            let row = fresh();
            items.update(|v| {
                let mid = v.len() / 2;
                v.insert(mid, row);
            });
        }))
        .child(el("button").id("remove-middle").on_click(move || {
            items.update(|v| {
                let mid = v.len() / 2;
                v.remove(mid);
            })
        }))
        .child(el("ul").each(move || items.get(), |row| row.key.clone(), item))
        .into()
}

fn item(row: Row) -> View {
    let draft = signal(String::new());
    el("li")
        .child(el("span").id(format!("label-{}", row.key)).child(text_fn(move || row.label.get())))
        .child(
            el("input")
                .id(format!("input-{}", row.key))
                .attr_fn("value", move || draft.get())
                .on_input(move |s| draft.set(s.to_string())),
        )
        .child(
            el("button")
                .id(format!("rename-{}", row.key))
                .on_click(move || row.label.update(|l| l.push('!'))),
        )
        .into()
}

fn list(keys: &[&str]) -> impl FnOnce() -> View {
    let initial = keys.iter().map(|k| (k.to_string(), k.to_uppercase())).collect();
    move || todo_list(initial)
}

fn label_order(app: &App) -> Vec<String> {
    app.ids()
        .into_iter()
        .filter_map(|id| id.strip_prefix("label-").map(str::to_string))
        .collect()
}

fn draft(app: &App, key: &str) -> String {
    app.attr(&format!("input-{key}"), "value").unwrap()
}

#[test]
fn local_state_follows_its_key_across_reverse_insert_and_remove() {
    let mut app = App::new(list(&["a", "b", "c"]));
    app.input("input-a", "alpha");
    app.input("input-c", "gamma");

    app.click("reverse");
    assert_eq!(label_order(&app), ["c", "b", "a"]);
    assert_eq!(draft(&app, "a"), "alpha");
    assert_eq!(draft(&app, "b"), "");
    assert_eq!(draft(&app, "c"), "gamma");

    app.click("insert-front");
    assert_eq!(label_order(&app), ["new0", "c", "b", "a"]);
    assert_eq!(draft(&app, "new0"), "");
    assert_eq!(draft(&app, "a"), "alpha");
    assert_eq!(draft(&app, "c"), "gamma");

    app.click("remove-first");
    app.click("remove-first");
    assert_eq!(label_order(&app), ["b", "a"]);
    assert_eq!(draft(&app, "a"), "alpha");
    assert_eq!(draft(&app, "b"), "");
    assert_eq!(app.text("label-a"), "A");
}

#[test]
fn local_state_follows_its_key_across_middle_insert_and_remove() {
    let mut app = App::new(list(&["a", "b", "c", "d"]));
    app.input("input-a", "alpha");
    app.input("input-c", "gamma");
    app.input("input-d", "delta");

    app.click("insert-middle");
    assert_eq!(label_order(&app), ["a", "b", "new0", "c", "d"]);
    assert_eq!(draft(&app, "new0"), "");
    assert_eq!(draft(&app, "c"), "gamma");
    assert_eq!(draft(&app, "d"), "delta");

    app.click("remove-middle");
    app.click("remove-middle");
    assert_eq!(label_order(&app), ["a", "b", "d"]);
    assert_eq!(draft(&app, "a"), "alpha");
    assert_eq!(draft(&app, "b"), "");
    assert_eq!(draft(&app, "d"), "delta");
}

fn hundred() -> impl FnOnce() -> View {
    let initial = (0..100).map(|i| (format!("k{i}"), format!("Item {i}"))).collect();
    move || todo_list(initial)
}

#[test]
fn renaming_one_item_of_a_hundred_reruns_only_its_label() {
    let mut app = App::new(hundred());
    app.click("rename-k50");
    assert_eq!(app.text("label-k50"), "Item 50!");
    assert_eq!(app.text("label-k49"), "Item 49");
    let stats = app.last_update();
    assert_eq!(stats.reruns, 1);
    assert_eq!(stats.nodes_changed, 1);
    assert_eq!(stats.elements_diffed, 0);
    println!("signals, rename 1 of 100: {stats:?}");
}

#[test]
fn typing_into_one_item_of_a_hundred_reruns_only_its_input() {
    let mut app = App::new(hundred());
    app.input("input-k50", "x");
    let stats = app.last_update();
    assert_eq!(stats.reruns, 1);
    assert_eq!(stats.nodes_changed, 1);
    println!("signals, type into 1 of 100: {stats:?}");
}
