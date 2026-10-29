use std::cell::Cell;
use std::rc::Rc;

use mountain_core::testing::{Command, PaintCounter, RecordingRenderer};
use mountain_core::{Color, Core, LogicalRect, NodeId, PhysicalRect, PhysicalSize, WindowEvent};

const RED: Color = Color {
    r: 255,
    g: 0,
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

fn size() -> PhysicalSize {
    PhysicalSize {
        width: 800,
        height: 600,
    }
}

/// A parent holding two children, each counting how often it paints.
struct Scene {
    core: Core,
    renderer: RecordingRenderer,
    parent: (NodeId, Rc<Cell<usize>>),
    first: (NodeId, Rc<Cell<usize>>),
    second: (NodeId, Rc<Cell<usize>>),
}

impl Scene {
    fn new() -> Scene {
        let mut core = Core::new();
        let root = core.tree().root();
        let parent = add(&mut core, root, logical(10.0, 10.0, 300.0, 300.0));
        let first = add(&mut core, parent.0, logical(0.0, 0.0, 50.0, 50.0));
        let second = add(&mut core, parent.0, logical(100.0, 0.0, 50.0, 50.0));
        let mut renderer = RecordingRenderer::default();
        core.handle(
            WindowEvent::Created {
                size: size(),
                scale_factor: 1.0,
            },
            &mut renderer,
        )
        .unwrap();
        let mut scene = Scene {
            core,
            renderer,
            parent,
            first,
            second,
        };
        scene.frame();
        scene
    }

    fn frame(&mut self) -> Vec<Command> {
        self.core
            .handle(WindowEvent::RedrawRequested, &mut self.renderer)
            .unwrap();
        self.renderer.frames.last().unwrap().commands.clone()
    }

    fn counts(&self) -> [usize; 3] {
        [self.parent.1.get(), self.first.1.get(), self.second.1.get()]
    }
}

fn add(core: &mut Core, parent: NodeId, bounds: LogicalRect) -> (NodeId, Rc<Cell<usize>>) {
    let count = Rc::new(Cell::new(0));
    let id = core
        .tree_mut()
        .add(parent, bounds, PaintCounter::new(RED, count.clone()));
    (id, count)
}

#[test]
fn every_host_node_paints_once_for_the_first_frame() {
    let scene = Scene::new();
    assert_eq!(scene.renderer.frames.len(), 1);
    assert_eq!(scene.counts(), [1, 1, 1]);
}

#[test]
fn an_unchanged_tree_paints_nothing_again_and_draws_the_same() {
    let mut scene = Scene::new();
    let first = scene.renderer.frames[0].commands.clone();
    assert_eq!(scene.frame(), first);
    assert_eq!(scene.counts(), [1, 1, 1]);
}

#[test]
fn an_updated_host_node_paints_again_and_its_siblings_do_not() {
    let mut scene = Scene::new();
    scene
        .core
        .tree_mut()
        .update(scene.first.0, |n: &mut PaintCounter| n.color = BLUE);
    assert_eq!(
        scene.frame(),
        [
            Command::FillRect(physical(10.0, 10.0, 300.0, 300.0), RED),
            Command::FillRect(physical(10.0, 10.0, 50.0, 50.0), BLUE),
            Command::FillRect(physical(110.0, 10.0, 50.0, 50.0), RED),
        ]
    );
    assert_eq!(scene.counts(), [1, 2, 1]);
}

#[test]
fn moving_a_host_node_paints_nothing_again() {
    let mut scene = Scene::new();
    scene
        .core
        .tree_mut()
        .set_bounds(scene.parent.0, logical(40.0, 50.0, 300.0, 300.0));
    assert_eq!(
        scene.frame(),
        [
            Command::FillRect(physical(40.0, 50.0, 300.0, 300.0), RED),
            Command::FillRect(physical(40.0, 50.0, 50.0, 50.0), RED),
            Command::FillRect(physical(140.0, 50.0, 50.0, 50.0), RED),
        ]
    );
    assert_eq!(scene.counts(), [1, 1, 1]);
}

#[test]
fn reordering_siblings_paints_nothing_again() {
    let mut scene = Scene::new();
    scene.core.tree_mut().move_to(scene.second.0, 0);
    assert_eq!(
        scene.frame(),
        [
            Command::FillRect(physical(10.0, 10.0, 300.0, 300.0), RED),
            Command::FillRect(physical(110.0, 10.0, 50.0, 50.0), RED),
            Command::FillRect(physical(10.0, 10.0, 50.0, 50.0), RED),
        ]
    );
    assert_eq!(scene.counts(), [1, 1, 1]);
}

#[test]
fn a_new_scale_factor_paints_nothing_again_and_scales_what_is_drawn() {
    let mut scene = Scene::new();
    let response = scene
        .core
        .handle(
            WindowEvent::ScaleFactorChanged {
                scale_factor: 2.0,
                size: size(),
            },
            &mut scene.renderer,
        )
        .unwrap();
    assert!(response.redraw);
    assert_eq!(
        scene.frame(),
        [
            Command::FillRect(physical(20.0, 20.0, 600.0, 600.0), RED),
            Command::FillRect(physical(20.0, 20.0, 100.0, 100.0), RED),
            Command::FillRect(physical(220.0, 20.0, 100.0, 100.0), RED),
        ]
    );
    assert_eq!(scene.counts(), [1, 1, 1]);
}

#[test]
fn a_host_node_added_later_paints_once_in_its_first_frame() {
    let mut scene = Scene::new();
    let (_, count) = add(
        &mut scene.core,
        scene.parent.0,
        logical(200.0, 0.0, 10.0, 10.0),
    );
    scene.frame();
    scene.frame();
    assert_eq!(count.get(), 1);
    assert_eq!(scene.counts(), [1, 1, 1]);
}

#[test]
fn resizing_a_host_node_paints_it_again_at_its_new_size() {
    for (bounds, drawn) in [
        (
            logical(100.0, 0.0, 80.0, 20.0),
            physical(110.0, 10.0, 80.0, 20.0),
        ),
        (
            logical(100.0, 0.0, 70.0, 50.0),
            physical(110.0, 10.0, 70.0, 50.0),
        ),
        (
            logical(100.0, 0.0, 50.0, 30.0),
            physical(110.0, 10.0, 50.0, 30.0),
        ),
    ] {
        let mut scene = Scene::new();
        scene.core.tree_mut().set_bounds(scene.second.0, bounds);
        assert_eq!(scene.frame()[2], Command::FillRect(drawn, RED));
        assert_eq!(scene.counts(), [1, 1, 2]);
    }
}

#[test]
fn a_change_that_panics_part_way_still_paints_the_node_again() {
    let mut scene = Scene::new();
    let id = scene.first.0;
    let tree = scene.core.tree_mut();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tree.update(id, |n: &mut PaintCounter| {
            n.color = BLUE;
            panic!("part-way");
        })
    }));
    assert!(result.is_err());
    assert_eq!(
        scene.frame()[1],
        Command::FillRect(physical(10.0, 10.0, 50.0, 50.0), BLUE)
    );
    assert_eq!(scene.counts(), [1, 2, 1]);
}

#[test]
fn changing_a_parent_paints_only_the_parent_again() {
    let mut scene = Scene::new();
    scene
        .core
        .tree_mut()
        .update(scene.parent.0, |n: &mut PaintCounter| n.color = BLUE);
    scene.frame();
    assert_eq!(scene.counts(), [2, 1, 1]);
    scene
        .core
        .tree_mut()
        .set_bounds(scene.parent.0, logical(10.0, 10.0, 200.0, 250.0));
    scene.frame();
    assert_eq!(scene.counts(), [3, 1, 1]);
}

#[test]
fn window_events_paint_nothing_again() {
    let mut scene = Scene::new();
    for event in [
        WindowEvent::Resized(PhysicalSize {
            width: 1024,
            height: 768,
        }),
        WindowEvent::Occluded(true),
        WindowEvent::Occluded(false),
        WindowEvent::RedrawRequested,
    ] {
        scene.core.handle(event, &mut scene.renderer).unwrap();
    }
    assert_eq!(scene.renderer.frames.len(), 2);
    assert_eq!(scene.counts(), [1, 1, 1]);
}
