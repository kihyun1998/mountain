use signals::*;

fn counter() -> View {
    let count = signal(0);
    el("div")
        .child(el("span").id("count").child(text_fn(move || count.get().to_string())))
        .child(
            el("button")
                .id("inc")
                .on_click(move || count.update(|n| *n += 1))
                .child(text("+")),
        )
        .into()
}

#[test]
fn counter_starts_at_zero_and_each_click_increments() {
    let mut app = App::new(counter);
    assert_eq!(app.text("count"), "0");
    app.click("inc");
    assert_eq!(app.text("count"), "1");
    app.click("inc");
    app.click("inc");
    assert_eq!(app.text("count"), "3");
}

#[test]
fn one_click_reruns_only_the_text_effect_and_changes_one_text_node() {
    let mut app = App::new(counter);
    app.click("inc");
    let stats = app.last_update();
    assert_eq!(stats.reruns, 1);
    assert_eq!(stats.nodes_changed, 1);
    println!("signals, counter click: {stats:?}");
}
