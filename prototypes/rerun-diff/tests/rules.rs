use rerun_diff::*;

#[derive(PartialEq)]
struct Show {
    n: i32,
}

impl Component for Show {
    fn render(&self, _cx: &mut Cx) -> El {
        el("span").id("shown").child(text(self.n.to_string())).into()
    }
}

#[derive(PartialEq)]
struct Parent;

impl Component for Parent {
    fn render(&self, cx: &mut Cx) -> El {
        let n = cx.use_state(|| 0);
        el("div")
            .child(el("button").id("inc").on_click(move || n.set(n.get() + 1)))
            .child(comp(Show { n: n.get() }))
            .into()
    }
}

#[test]
fn a_child_given_parent_state_as_a_value_prop_follows_it() {
    let mut app = App::new(Parent);
    app.click("inc");
    app.click("inc");
    assert_eq!(app.text("shown"), "2");
    assert_eq!(app.last_update().reruns, 2);
}

#[derive(PartialEq)]
struct Sum;

impl Component for Sum {
    fn render(&self, cx: &mut Cx) -> El {
        let a = cx.use_state(|| 1);
        let b = cx.use_state(|| 2);
        let total = a.with(|x| x + b.get());
        el("span").id("sum").child(text(total.to_string())).into()
    }
}

#[test]
fn reading_one_state_inside_another_states_with_works() {
    let app = App::new(Sum);
    assert_eq!(app.text("sum"), "3");
}

#[derive(PartialEq)]
struct Conditional;

impl Component for Conditional {
    fn render(&self, cx: &mut Cx) -> El {
        let on = cx.use_state(|| false);
        if on.get() {
            let _extra = cx.use_state(|| "x".to_string());
        }
        el("button").id("toggle").on_click(move || on.set(true)).into()
    }
}

#[test]
#[should_panic(expected = "hook order changed")]
fn calling_a_hook_conditionally_is_rejected() {
    let mut app = App::new(Conditional);
    app.click("toggle");
}

#[derive(PartialEq)]
struct Keyed {
    user: u32,
}

impl Component for Keyed {
    fn render(&self, _cx: &mut Cx) -> El {
        comp_keyed(format!("user-{}", self.user), Draft)
    }
}

#[derive(PartialEq)]
struct Draft;

impl Component for Draft {
    fn render(&self, cx: &mut Cx) -> El {
        let draft = cx.use_state(String::new);
        el("input")
            .id("draft")
            .attr("value", draft.get())
            .on_input(move |s| draft.set(s.to_string()))
            .into()
    }
}

#[derive(PartialEq)]
struct SwitchUser;

impl Component for SwitchUser {
    fn render(&self, cx: &mut Cx) -> El {
        let user = cx.use_state(|| 1);
        el("div")
            .child(el("button").id("next-user").on_click(move || user.set(user.get() + 1)))
            .child(comp(Keyed { user: user.get() }))
            .into()
    }
}

#[test]
fn changing_the_key_of_a_component_output_resets_its_state() {
    let mut app = App::new(SwitchUser);
    app.input("draft", "hello");
    app.click("next-user");
    assert_eq!(app.attr("draft", "value").unwrap(), "");
}

#[derive(PartialEq)]
struct Twins;

impl Component for Twins {
    fn render(&self, _cx: &mut Cx) -> El {
        el("ul")
            .child(el("li").key("a"))
            .child(el("li").key("a"))
            .into()
    }
}

#[test]
#[should_panic(expected = "duplicate key")]
fn two_children_with_the_same_key_are_rejected() {
    App::new(Twins);
}

#[derive(PartialEq)]
struct Peek {
    s: Setter<i32>,
}

impl Component for Peek {
    fn render(&self, _cx: &mut Cx) -> El {
        let mut seen = 0;
        self.s.update(|v| seen = *v);
        el("span").id("peek").child(text(seen.to_string())).into()
    }
}

#[derive(PartialEq)]
struct PeekParent;

impl Component for PeekParent {
    fn render(&self, cx: &mut Cx) -> El {
        let n = cx.use_state(|| 0);
        comp(Peek { s: n.setter() })
    }
}

#[test]
#[should_panic(expected = "while rendering")]
fn writing_through_a_setter_during_render_is_rejected() {
    App::new(PeekParent);
}

#[derive(PartialEq)]
struct Fragile;

impl Component for Fragile {
    fn render(&self, cx: &mut Cx) -> El {
        let n = cx.use_state(|| 0);
        el("div")
            .child(el("span").id("n").child(text(n.get().to_string())))
            .child(el("button").id("boom").on_click(move || n.with(|_| panic!("reader failed"))))
            .child(el("button").id("inc").on_click(move || n.set(n.get() + 1)))
            .into()
    }
}

#[test]
fn a_state_is_usable_after_a_reader_panicked() {
    let mut app = App::new(Fragile);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.click("boom")));
    assert!(caught.is_err());
    app.click("inc");
    assert_eq!(app.text("n"), "1");
}
