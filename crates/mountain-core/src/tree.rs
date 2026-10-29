//! The tree of Host nodes the Core draws, and the Display lists they paint.

use std::any::Any;
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Color, LogicalRect, LogicalSize};

/// The identity of one Host node, never given to another (ADR-0006).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(NonZeroU64);

/// The last Node id handed out, across every `Tree` in the process.
static LAST_NODE_ID: AtomicU64 = AtomicU64::new(0);

impl NodeId {
    fn next() -> NodeId {
        let id = LAST_NODE_ID.fetch_add(1, Ordering::Relaxed) + 1;
        NodeId(NonZeroU64::new(id).expect("the Node id counter starts at 1"))
    }

    pub fn get(self) -> u64 {
        self.0.get()
    }
}

/// A node the framework lays out and draws.
pub trait HostNode: Any {
    /// Records how this node looks at `size`, in its own coordinates and in
    /// logical pixels.
    fn paint(&self, size: LogicalSize, list: &mut DisplayList);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DrawCommand {
    FillRect(LogicalRect, Color),
}

/// The drawing commands a Host node records when it paints.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayList {
    commands: Vec<DrawCommand>,
}

impl DisplayList {
    pub fn new() -> DisplayList {
        DisplayList::default()
    }

    pub fn fill_rect(&mut self, rect: LogicalRect, color: Color) {
        self.commands.push(DrawCommand::FillRect(rect, color));
    }

    pub(crate) fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }
}

/// A Host node that fills its box with one colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColoredBox {
    pub color: Color,
}

impl ColoredBox {
    pub fn new(color: Color) -> ColoredBox {
        ColoredBox { color }
    }
}

impl HostNode for ColoredBox {
    fn paint(&self, size: LogicalSize, list: &mut DisplayList) {
        let whole = LogicalRect {
            x: 0.0,
            y: 0.0,
            width: size.width,
            height: size.height,
        };
        list.fill_rect(whole, self.color);
    }
}

/// The root Host node: it paints nothing.
struct Root;

impl HostNode for Root {
    fn paint(&self, _size: LogicalSize, _list: &mut DisplayList) {}
}

struct Entry {
    node: Box<dyn HostNode>,
    /// Position relative to the parent, and size, in logical pixels.
    bounds: LogicalRect,
    /// `None` only for the root.
    parent: Option<NodeId>,
    children: Vec<NodeId>,
}

impl Entry {
    fn new(node: Box<dyn HostNode>, bounds: LogicalRect, parent: Option<NodeId>) -> Entry {
        Entry {
            node,
            bounds,
            parent,
            children: Vec::new(),
        }
    }
}

/// The Host nodes the Core draws, from a root that exists from the start at
/// the window's origin.
///
/// This is the low-level API the state model is to drive; until then an app
/// builds and changes its Host nodes through it directly.
pub struct Tree {
    entries: HashMap<NodeId, Entry>,
    root: NodeId,
}

impl Default for Tree {
    fn default() -> Tree {
        Tree::new()
    }
}

impl Tree {
    pub fn new() -> Tree {
        let root = NodeId::next();
        let mut entries = HashMap::new();
        let origin = LogicalRect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
        entries.insert(root, Entry::new(Box::new(Root), origin, None));
        Tree { entries, root }
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Adds `node` as the last child of `parent`, at `bounds` relative to
    /// the parent, and returns its Node id.
    ///
    /// # Panics
    ///
    /// If `parent` is not a Host node in this tree.
    pub fn add(&mut self, parent: NodeId, bounds: LogicalRect, node: impl HostNode) -> NodeId {
        let id = NodeId::next();
        self.entry_mut(parent).children.push(id);
        self.entries
            .insert(id, Entry::new(Box::new(node), bounds, Some(parent)));
        id
    }

    /// Sets the position relative to the parent, and the size, of `id`.
    ///
    /// # Panics
    ///
    /// If `id` is not a Host node in this tree, or is the root.
    pub fn set_bounds(&mut self, id: NodeId, bounds: LogicalRect) {
        let entry = self.entry_mut(id);
        assert!(
            entry.parent.is_some(),
            "the root stays at the window's origin"
        );
        entry.bounds = bounds;
    }

    /// Whether `id` is a Host node in this tree.
    pub fn contains(&self, id: NodeId) -> bool {
        self.entries.contains_key(&id)
    }

    /// Hands the Host node `id` to `change` as a `T`. This is the one way to
    /// change a Host node's values.
    ///
    /// # Panics
    ///
    /// If `id` is not a Host node in this tree, or is not a `T`.
    pub fn update<T: HostNode>(&mut self, id: NodeId, change: impl FnOnce(&mut T)) {
        let entry = self.entry_mut(id);
        let node: &mut dyn Any = entry.node.as_mut();
        match node.downcast_mut::<T>() {
            Some(node) => change(node),
            None => panic!("{id:?} is not a {}", std::any::type_name::<T>()),
        }
    }

    /// Removes `id` and every Host node under it.
    ///
    /// # Panics
    ///
    /// If `id` is not a Host node in this tree, or is the root.
    pub fn remove(&mut self, id: NodeId) {
        let parent = self.parent_of(id, "the root cannot be removed");
        self.entry_mut(parent).children.retain(|&child| child != id);
        let mut doomed = vec![id];
        while let Some(next) = doomed.pop() {
            let entry = self.entries.remove(&next).expect("a child is in the tree");
            doomed.extend(entry.children);
        }
    }

    /// Moves `id` to `index` among its parent's children, which sets where
    /// it is drawn among its siblings.
    ///
    /// # Panics
    ///
    /// If `id` is not a Host node in this tree, is the root, or `index` is
    /// past the last of its parent's children.
    pub fn move_to(&mut self, id: NodeId, index: usize) {
        let parent = self.parent_of(id, "the root has no siblings");
        let children = &mut self.entry_mut(parent).children;
        let count = children.len();
        assert!(
            index < count,
            "index {index} is past the last of the {count} children of {id:?}'s parent"
        );
        children.retain(|&child| child != id);
        children.insert(index, id);
    }

    /// The parent of `id`, or a panic with `root_message` if `id` is the root.
    fn parent_of(&self, id: NodeId, root_message: &str) -> NodeId {
        match self.entry(id).parent {
            Some(parent) => parent,
            None => panic!("{root_message}"),
        }
    }

    fn entry(&self, id: NodeId) -> &Entry {
        match self.entries.get(&id) {
            Some(entry) => entry,
            None => panic!("{id:?} is not a Host node in this tree"),
        }
    }

    fn entry_mut(&mut self, id: NodeId) -> &mut Entry {
        match self.entries.get_mut(&id) {
            Some(entry) => entry,
            None => panic!("{id:?} is not a Host node in this tree"),
        }
    }

    /// Calls `draw` with each Host node's Display list and the window
    /// position of the node's origin, parents before children and children
    /// in order.
    pub(crate) fn paint(&self, draw: &mut dyn FnMut(&DisplayList, f64, f64)) {
        self.paint_from(self.root, 0.0, 0.0, draw);
    }

    fn paint_from(
        &self,
        id: NodeId,
        parent_x: f64,
        parent_y: f64,
        draw: &mut dyn FnMut(&DisplayList, f64, f64),
    ) {
        let entry = self.entry(id);
        let x = parent_x + entry.bounds.x;
        let y = parent_y + entry.bounds.y;
        let mut list = DisplayList::new();
        let size = LogicalSize {
            width: entry.bounds.width,
            height: entry.bounds.height,
        };
        entry.node.paint(size, &mut list);
        draw(&list, x, y);
        for &child in &entry.children {
            self.paint_from(child, x, y, draw);
        }
    }
}
