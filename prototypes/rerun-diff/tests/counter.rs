use rerun_diff::*;

#[derive(PartialEq)]
struct Counter;

impl Component for Counter {
    fn render(&self, cx: &mut Cx) -> El {
        let count = cx.use_state(|| 0);
        el("div")
            .child(el("span").id("count").child(text(count.get().to_string())))
            .child(
                el("button")
                    .id("inc")
                    .on_click(move || count.set(count.get() + 1))
                    .child(text("+")),
            )
            .into()
    }
}

#[test]
fn counter_starts_at_zero_and_each_click_increments() {
    let mut app = App::new(Counter);
    assert_eq!(app.text("count"), "0");
    app.click("inc");
    assert_eq!(app.text("count"), "1");
    app.click("inc");
    app.click("inc");
    assert_eq!(app.text("count"), "3");
}

#[test]
fn one_click_reruns_only_the_counter_and_changes_one_text_node() {
    let mut app = App::new(Counter);
    app.click("inc");
    let stats = app.last_update();
    assert_eq!(stats.reruns, 1);
    assert_eq!(stats.nodes_changed, 1);
    println!("rerun-diff, counter click: {stats:?}");
}
