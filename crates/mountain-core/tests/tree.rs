use mountain_core::testing::{Command, TestPlatform};
use mountain_core::{
    Color, ColoredBox, Core, DisplayList, HostNode, LogicalRect, LogicalSize, PhysicalRect,
    PhysicalSize, Platform, Tree, WindowEvent,
};

const WHITE: Color = Color {
    r: 255,
    g: 255,
    b: 255,
    a: 255,
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

fn first_frame(core: &mut Core, scale_factor: f64) -> Vec<Command> {
    let mut platform = TestPlatform::new(vec![WindowEvent::Created {
        size: PhysicalSize {
            width: 800,
            height: 600,
        },
        scale_factor,
    }]);
    platform.run(core).unwrap();
    platform.renderer.frames.remove(0).commands
}

#[test]
fn a_colored_box_is_drawn_at_its_place_in_physical_pixels() {
    let mut core = Core::new();
    let root = core.tree().root();
    core.tree_mut()
        .add(root, logical(10.0, 20.0, 100.0, 50.0), ColoredBox::new(RED));
    assert_eq!(
        first_frame(&mut core, 2.0),
        [Command::FillRect(physical(20.0, 40.0, 200.0, 100.0), RED)]
    );
}

#[test]
fn a_nested_box_is_offset_by_every_ancestor() {
    let mut core = Core::new();
    let root = core.tree().root();
    let outer = core.tree_mut().add(
        root,
        logical(10.0, 10.0, 300.0, 200.0),
        ColoredBox::new(RED),
    );
    let middle = core.tree_mut().add(
        outer,
        logical(20.0, 30.0, 100.0, 100.0),
        ColoredBox::new(GREEN),
    );
    core.tree_mut()
        .add(middle, logical(5.0, 7.0, 10.0, 10.0), ColoredBox::new(BLUE));
    assert_eq!(
        first_frame(&mut core, 1.0),
        [
            Command::FillRect(physical(10.0, 10.0, 300.0, 200.0), RED),
            Command::FillRect(physical(30.0, 40.0, 100.0, 100.0), GREEN),
            Command::FillRect(physical(35.0, 47.0, 10.0, 10.0), BLUE),
        ]
    );
}

#[test]
fn the_background_comes_first_then_each_parent_before_its_children_in_order() {
    let mut core = Core::new().with_background(WHITE);
    let root = core.tree().root();
    let first = core
        .tree_mut()
        .add(root, logical(0.0, 0.0, 100.0, 100.0), ColoredBox::new(RED));
    core.tree_mut().add(
        root,
        logical(50.0, 0.0, 100.0, 100.0),
        ColoredBox::new(BLUE),
    );
    core.tree_mut().add(
        first,
        logical(10.0, 10.0, 20.0, 20.0),
        ColoredBox::new(GREEN),
    );
    assert_eq!(
        first_frame(&mut core, 1.0),
        [
            Command::FillRect(physical(0.0, 0.0, 800.0, 600.0), WHITE),
            Command::FillRect(physical(0.0, 0.0, 100.0, 100.0), RED),
            Command::FillRect(physical(10.0, 10.0, 20.0, 20.0), GREEN),
            Command::FillRect(physical(50.0, 0.0, 100.0, 100.0), BLUE),
        ]
    );
}

#[test]
fn each_added_host_node_gets_its_own_node_id() {
    let mut core = Core::new();
    let root = core.tree().root();
    let a = core
        .tree_mut()
        .add(root, logical(0.0, 0.0, 1.0, 1.0), ColoredBox::new(RED));
    let b = core
        .tree_mut()
        .add(root, logical(0.0, 0.0, 1.0, 1.0), ColoredBox::new(RED));
    let c = core
        .tree_mut()
        .add(a, logical(0.0, 0.0, 1.0, 1.0), ColoredBox::new(RED));
    let ids = [root, a, b, c];
    for (i, x) in ids.iter().enumerate() {
        assert_ne!(x.get(), 0);
        for y in &ids[i + 1..] {
            assert_ne!(x, y);
        }
    }
}

/// Paints a bar across the top quarter of its box, in its own coordinates.
struct TopBar(Color);

impl HostNode for TopBar {
    fn paint(&self, size: LogicalSize, list: &mut DisplayList) {
        list.fill_rect(logical(0.0, 0.0, size.width, size.height / 4.0), self.0);
    }
}

#[test]
fn a_host_node_from_outside_the_core_paints_in_its_own_coordinates() {
    let mut core = Core::new();
    let root = core.tree().root();
    let parent = core.tree_mut().add(
        root,
        logical(100.0, 100.0, 400.0, 400.0),
        ColoredBox::new(RED),
    );
    core.tree_mut()
        .add(parent, logical(10.0, 20.0, 200.0, 80.0), TopBar(BLUE));
    assert_eq!(
        first_frame(&mut core, 1.5),
        [
            Command::FillRect(physical(150.0, 150.0, 600.0, 600.0), RED),
            Command::FillRect(physical(165.0, 180.0, 300.0, 30.0), BLUE),
        ]
    );
}

#[test]
fn moving_a_parent_with_set_bounds_moves_its_children() {
    let mut core = Core::new();
    let root = core.tree().root();
    let parent = core
        .tree_mut()
        .add(root, logical(0.0, 0.0, 100.0, 100.0), ColoredBox::new(RED));
    core.tree_mut().add(
        parent,
        logical(10.0, 10.0, 20.0, 20.0),
        ColoredBox::new(BLUE),
    );
    core.tree_mut()
        .set_bounds(parent, logical(50.0, 60.0, 200.0, 100.0));
    assert_eq!(
        first_frame(&mut core, 1.0),
        [
            Command::FillRect(physical(50.0, 60.0, 200.0, 100.0), RED),
            Command::FillRect(physical(60.0, 70.0, 20.0, 20.0), BLUE),
        ]
    );
}

#[test]
fn host_nodes_in_different_trees_never_share_a_node_id() {
    let mut first = Core::new();
    let mut second = Core::new();
    let mut ids = Vec::new();
    for core in [&mut first, &mut second] {
        let root = core.tree().root();
        ids.push(root);
        ids.push(
            core.tree_mut()
                .add(root, logical(0.0, 0.0, 1.0, 1.0), ColoredBox::new(RED)),
        );
    }
    *first.tree_mut() = Tree::new();
    let root = first.tree().root();
    ids.push(root);
    ids.push(
        first
            .tree_mut()
            .add(root, logical(0.0, 0.0, 1.0, 1.0), ColoredBox::new(RED)),
    );
    for (i, x) in ids.iter().enumerate() {
        for y in &ids[i + 1..] {
            assert_ne!(x, y);
        }
    }
}

#[test]
#[should_panic(expected = "is not a Host node in this tree")]
fn a_node_id_from_another_tree_panics() {
    let mut other = Tree::new();
    let root = other.root();
    let foreign = other.add(root, logical(0.0, 0.0, 1.0, 1.0), ColoredBox::new(RED));
    let mut tree = Tree::new();
    let root = tree.root();
    tree.add(root, logical(0.0, 0.0, 1.0, 1.0), ColoredBox::new(RED));
    tree.set_bounds(foreign, logical(0.0, 0.0, 2.0, 2.0));
}

/// Paints two overlapping rectangles, the second over the first.
struct TwoBars;

impl HostNode for TwoBars {
    fn paint(&self, _size: LogicalSize, list: &mut DisplayList) {
        list.fill_rect(logical(0.0, 0.0, 10.0, 10.0), RED);
        list.fill_rect(logical(5.0, 5.0, 10.0, 10.0), BLUE);
    }
}

#[test]
fn a_host_nodes_commands_are_replayed_in_the_order_it_recorded_them() {
    let mut core = Core::new();
    let root = core.tree().root();
    core.tree_mut()
        .add(root, logical(100.0, 0.0, 50.0, 50.0), TwoBars);
    assert_eq!(
        first_frame(&mut core, 1.0),
        [
            Command::FillRect(physical(100.0, 0.0, 10.0, 10.0), RED),
            Command::FillRect(physical(105.0, 5.0, 10.0, 10.0), BLUE),
        ]
    );
}

#[test]
fn a_child_outside_its_parent_is_still_drawn_whole() {
    let mut core = Core::new();
    let root = core.tree().root();
    let parent = core
        .tree_mut()
        .add(root, logical(10.0, 10.0, 20.0, 20.0), ColoredBox::new(RED));
    core.tree_mut().add(
        parent,
        logical(30.0, -5.0, 40.0, 40.0),
        ColoredBox::new(BLUE),
    );
    assert_eq!(
        first_frame(&mut core, 1.0),
        [
            Command::FillRect(physical(10.0, 10.0, 20.0, 20.0), RED),
            Command::FillRect(physical(40.0, 5.0, 40.0, 40.0), BLUE),
        ]
    );
}
