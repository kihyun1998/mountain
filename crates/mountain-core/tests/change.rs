use mountain_core::testing::{Command, RecordingRenderer};
use mountain_core::{
    Color, ColoredBox, Core, DisplayList, HostNode, LogicalRect, LogicalSize, NodeId, PhysicalRect,
    PhysicalSize, Tree, WindowEvent,
};

const RED: Color = Color {
    r: 255,
    g: 0,
    b: 0,
    a: 255,
};
const GREEN: Color = Color {
    r: 0,
    g: 255,
    b: 0,
    a: 255,
};
const BLUE: Color = Color {
    r: 0,
    g: 0,
    b: 255,
    a: 255,
};

fn logical(x: f64, y: f64, width: f64, height: f64) -> LogicalRect {
    LogicalRect {
        x,
        y,
        width,
        height,
    }
}

fn physical(x: f64, y: f64, width: f64, height: f64) -> PhysicalRect {
    PhysicalRect {
        x,
        y,
        width,
        height,
    }
}

/// A Core with a window, a red box under the root holding a green box, and
/// a blue box after the red one.
struct Scene {
    core: Core,
    renderer: RecordingRenderer,
    red: NodeId,
    green: NodeId,
    blue: NodeId,
}

impl Scene {
    fn new() -> Scene {
        let mut core = Core::new();
        let root = core.tree().root();
        let red = core
            .tree_mut()
            .add(root, logical(0.0, 0.0, 100.0, 100.0), ColoredBox::new(RED));
        let green =
            core.tree_mut()
                .add(red, logical(10.0, 10.0, 20.0, 20.0), ColoredBox::new(GREEN));
        let blue = core.tree_mut().add(
            root,
            logical(50.0, 50.0, 100.0, 100.0),
            ColoredBox::new(BLUE),
        );
        let mut renderer = RecordingRenderer::default();
        core.handle(
            WindowEvent::Created {
                size: PhysicalSize {
                    width: 800,
                    height: 600,
                },
                scale_factor: 1.0,
            },
            &mut renderer,
        )
        .unwrap();
        Scene {
            core,
            renderer,
            red,
            green,
            blue,
        }
    }

    fn frame(&mut self) -> Vec<Command> {
        self.core
            .handle(WindowEvent::RedrawRequested, &mut self.renderer)
            .unwrap();
        self.renderer.frames.last().unwrap().commands.clone()
    }
}

#[test]
fn a_changed_colour_is_drawn_on_the_next_frame() {
    let mut scene = Scene::new();
    scene.frame();
    scene
        .core
        .tree_mut()
        .update(scene.green, |b: &mut ColoredBox| b.color = BLUE);
    assert_eq!(
        scene.frame(),
        [
            Command::FillRect(physical(0.0, 0.0, 100.0, 100.0), RED),
            Command::FillRect(physical(10.0, 10.0, 20.0, 20.0), BLUE),
            Command::FillRect(physical(50.0, 50.0, 100.0, 100.0), BLUE),
        ]
    );
}

#[test]
fn removing_a_host_node_removes_its_subtree_from_the_next_frame() {
    let mut scene = Scene::new();
    scene.frame();
    scene.core.tree_mut().remove(scene.red);
    assert_eq!(
        scene.frame(),
        [Command::FillRect(physical(50.0, 50.0, 100.0, 100.0), BLUE)]
    );
}

#[test]
fn moving_a_host_node_among_its_siblings_changes_the_drawing_order() {
    let mut scene = Scene::new();
    scene.frame();
    scene.core.tree_mut().move_to(scene.blue, 0);
    assert_eq!(
        scene.frame(),
        [
            Command::FillRect(physical(50.0, 50.0, 100.0, 100.0), BLUE),
            Command::FillRect(physical(0.0, 0.0, 100.0, 100.0), RED),
            Command::FillRect(physical(10.0, 10.0, 20.0, 20.0), GREEN),
        ]
    );
}

#[test]
fn moving_a_host_node_later_among_its_siblings_draws_it_later() {
    let mut scene = Scene::new();
    let root = scene.core.tree().root();
    scene.core.tree_mut().add(
        root,
        logical(200.0, 0.0, 10.0, 10.0),
        ColoredBox::new(GREEN),
    );
    scene.frame();
    scene.core.tree_mut().move_to(scene.red, 1);
    assert_eq!(
        scene.frame(),
        [
            Command::FillRect(physical(50.0, 50.0, 100.0, 100.0), BLUE),
            Command::FillRect(physical(0.0, 0.0, 100.0, 100.0), RED),
            Command::FillRect(physical(10.0, 10.0, 20.0, 20.0), GREEN),
            Command::FillRect(physical(200.0, 0.0, 10.0, 10.0), GREEN),
        ]
    );
}

#[test]
fn a_removed_host_node_and_its_descendants_are_no_longer_live() {
    let mut scene = Scene::new();
    let tree = scene.core.tree_mut();
    let root = tree.root();
    assert!(tree.contains(scene.red) && tree.contains(scene.green));
    tree.remove(scene.red);
    assert!(!tree.contains(scene.red));
    assert!(!tree.contains(scene.green));
    assert!(tree.contains(scene.blue));
    assert!(tree.contains(root));
    let (tree, grandchild) = removed_grandchild();
    assert!(!tree.contains(grandchild));
}

#[test]
fn node_ids_added_after_a_removal_differ_from_every_removed_one() {
    let mut scene = Scene::new();
    let tree = scene.core.tree_mut();
    let root = tree.root();
    tree.remove(scene.red);
    for _ in 0..10 {
        let id = tree.add(root, logical(0.0, 0.0, 1.0, 1.0), ColoredBox::new(RED));
        assert_ne!(id, scene.red);
        assert_ne!(id, scene.green);
    }
}

/// A tree where a node, its child and its grandchild were added and the node
/// was then removed; returns the tree and the grandchild's Node id.
fn removed_grandchild() -> (Tree, NodeId) {
    let mut tree = Tree::new();
    let root = tree.root();
    let node = tree.add(root, logical(0.0, 0.0, 1.0, 1.0), ColoredBox::new(RED));
    let child = tree.add(node, logical(0.0, 0.0, 1.0, 1.0), ColoredBox::new(RED));
    let grandchild = tree.add(child, logical(0.0, 0.0, 1.0, 1.0), ColoredBox::new(RED));
    tree.add(root, logical(0.0, 0.0, 1.0, 1.0), ColoredBox::new(RED));
    tree.remove(node);
    (tree, grandchild)
}

fn panic_message(f: impl FnOnce()) -> String {
    let payload =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).expect_err("expected a panic");
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => payload.downcast::<&str>().unwrap().to_string(),
    }
}

type Operation = fn(&mut Tree, NodeId);

#[test]
fn every_tree_operation_on_a_removed_node_id_panics_naming_the_id() {
    let operations: [(&str, Operation); 5] = [
        ("add", |tree, id| {
            tree.add(id, logical(0.0, 0.0, 1.0, 1.0), ColoredBox::new(RED));
        }),
        ("set_bounds", |tree, id| {
            tree.set_bounds(id, logical(0.0, 0.0, 1.0, 1.0))
        }),
        ("update", |tree, id| {
            tree.update(id, |_: &mut ColoredBox| {})
        }),
        ("remove", |tree, id| tree.remove(id)),
        ("move_to", |tree, id| tree.move_to(id, 0)),
    ];
    for (operation, call) in operations {
        let (mut tree, id) = removed_grandchild();
        let message = panic_message(|| call(&mut tree, id));
        assert!(
            message.contains(&format!("{id:?}"))
                && message.contains("is not a Host node in this tree"),
            "{operation}: {message}"
        );
    }
}

struct Other;

impl HostNode for Other {
    fn paint(&self, _size: LogicalSize, _list: &mut DisplayList) {}
}

#[test]
#[should_panic(expected = "ColoredBox")]
fn updating_a_host_node_as_the_wrong_type_panics() {
    let mut core = Core::new();
    let root = core.tree().root();
    let id = core
        .tree_mut()
        .add(root, logical(0.0, 0.0, 1.0, 1.0), Other);
    core.tree_mut().update(id, |_: &mut ColoredBox| {});
}

#[test]
#[should_panic(expected = "the root cannot be removed")]
fn removing_the_root_panics() {
    let mut core = Core::new();
    let root = core.tree().root();
    core.tree_mut().remove(root);
}

#[test]
#[should_panic(expected = "the root stays at the window's origin")]
fn setting_the_roots_bounds_panics() {
    let mut core = Core::new();
    let root = core.tree().root();
    core.tree_mut()
        .set_bounds(root, logical(5.0, 5.0, 1.0, 1.0));
}

#[test]
#[should_panic(expected = "the root has no siblings")]
fn moving_the_root_panics() {
    let mut core = Core::new();
    let root = core.tree().root();
    core.tree_mut().move_to(root, 0);
}

#[test]
#[should_panic(expected = "past the last of the 2 children")]
fn moving_past_the_last_sibling_panics() {
    let mut scene = Scene::new();
    scene.core.tree_mut().move_to(scene.red, 2);
}
