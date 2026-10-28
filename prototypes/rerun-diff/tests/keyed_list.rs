use rerun_diff::*;

type Items = Vec<(String, String)>;

#[derive(PartialEq)]
struct TodoList {
    initial: Items,
}

impl Component for TodoList {
    fn render(&self, cx: &mut Cx) -> El {
        let items = cx.use_state(|| self.initial.clone());
        let next = cx.use_state(|| 0);
        el("div")
            .child(el("button").id("reverse").on_click(move || items.update(|v| v.reverse())))
            .child(el("button").id("insert-front").on_click(move || {
                let n = next.get();
                next.set(n + 1);
                items.update(|v| v.insert(0, (format!("new{n}"), format!("New {n}"))));
            }))
            .child(el("button").id("remove-first").on_click(move || {
                items.update(|v| {
                    v.remove(0);
                })
            }))
            .child(el("button").id("insert-middle").on_click(move || {
                let n = next.get();
                next.set(n + 1);
                items.update(|v| {
                    let mid = v.len() / 2;
                    v.insert(mid, (format!("new{n}"), format!("New {n}")));
                });
            }))
            .child(el("button").id("remove-middle").on_click(move || {
                items.update(|v| {
                    let mid = v.len() / 2;
                    v.remove(mid);
                })
            }))
            .child(el("ul").children(items.get().into_iter().map(|(key, label)| {
                comp_keyed(key.clone(), Item { key, label, items: items.setter() })
            })))
            .into()
    }
}

#[derive(PartialEq)]
struct Item {
    key: String,
    label: String,
    items: Setter<Items>,
}

impl Component for Item {
    fn render(&self, cx: &mut Cx) -> El {
        let draft = cx.use_state(String::new);
        let key = self.key.clone();
        let items = self.items;
        el("li")
            .child(el("span").id(format!("label-{}", self.key)).child(text(self.label.clone())))
            .child(
                el("input")
                    .id(format!("input-{}", self.key))
                    .attr("value", draft.get())
                    .on_input(move |s| draft.set(s.to_string())),
            )
            .child(el("button").id(format!("rename-{}", self.key)).on_click(move || {
                items.update(|v| {
                    if let Some(item) = v.iter_mut().find(|item| item.0 == key) {
                        item.1.push('!');
                    }
                })
            }))
            .into()
    }
}

fn list(keys: &[&str]) -> TodoList {
    TodoList {
        initial: keys.iter().map(|k| (k.to_string(), k.to_uppercase())).collect(),
    }
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

fn hundred() -> TodoList {
    TodoList {
        initial: (0..100).map(|i| (format!("k{i}"), format!("Item {i}"))).collect(),
    }
}

#[test]
fn renaming_one_item_of_a_hundred_reruns_the_list_and_that_item() {
    let mut app = App::new(hundred());
    app.click("rename-k50");
    assert_eq!(app.text("label-k50"), "Item 50!");
    assert_eq!(app.text("label-k49"), "Item 49");
    let stats = app.last_update();
    assert_eq!(stats.reruns, 2);
    assert_eq!(stats.nodes_changed, 1);
    assert!(stats.elements_diffed >= 100, "diffed {}", stats.elements_diffed);
    println!("rerun-diff, rename 1 of 100: {stats:?}");
}

#[test]
fn typing_into_one_item_of_a_hundred_reruns_only_that_item() {
    let mut app = App::new(hundred());
    app.input("input-k50", "x");
    let stats = app.last_update();
    assert_eq!(stats.reruns, 1);
    assert_eq!(stats.nodes_changed, 1);
    println!("rerun-diff, type into 1 of 100: {stats:?}");
}
