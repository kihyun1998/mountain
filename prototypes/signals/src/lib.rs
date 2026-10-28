//! Headless prototype of the fine-grained signals state model.
//!
//! Component functions run once. Reading a signal inside an effect subscribes
//! that effect; writing the signal re-runs only its subscribers, which patch
//! the nodes they own.

use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::Hash;
use std::marker::PhantomData;
use std::rc::Rc;

type NodeId = usize;
type ScopeId = usize;
type ClickFn = Rc<dyn Fn()>;
type InputFn = Rc<dyn Fn(&str)>;
type EffectFn = Rc<dyn Fn()>;
type StringFn = Rc<dyn Fn() -> String>;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
struct Handle {
    index: usize,
    generation: u32,
}

/// A slot store whose handles carry a generation, so a stale handle is caught.
struct Arena<T> {
    items: Vec<Option<T>>,
    gens: Vec<u32>,
    free: Vec<usize>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena { items: Vec::new(), gens: Vec::new(), free: Vec::new() }
    }
}

impl<T> Arena<T> {
    fn insert(&mut self, item: T) -> Handle {
        if let Some(index) = self.free.pop() {
            self.items[index] = Some(item);
            Handle { index, generation: self.gens[index] }
        } else {
            self.items.push(Some(item));
            self.gens.push(0);
            Handle { index: self.items.len() - 1, generation: 0 }
        }
    }
    fn get(&self, h: Handle) -> Option<&T> {
        if self.gens[h.index] == h.generation { self.items[h.index].as_ref() } else { None }
    }
    fn get_mut(&mut self, h: Handle) -> Option<&mut T> {
        if self.gens[h.index] == h.generation { self.items[h.index].as_mut() } else { None }
    }
    fn remove(&mut self, h: Handle) -> Option<T> {
        if self.gens[h.index] != h.generation {
            return None;
        }
        let item = self.items[h.index].take();
        self.gens[h.index] += 1;
        self.free.push(h.index);
        item
    }
}

// ---- Runtime ----------------------------------------------------------------

struct SignalSlot {
    value: Box<dyn Any>,
    subscribers: BTreeSet<(u64, Handle)>,
}

struct Effect {
    run: EffectFn,
    deps: Vec<Handle>,
    order: u64,
}

#[derive(Default)]
struct Scope {
    parent: Option<ScopeId>,
    signals: Vec<Handle>,
    effects: Vec<Handle>,
    children: Vec<ScopeId>,
    nodes: Vec<NodeId>,
}

enum Node {
    Element {
        tag: &'static str,
        attrs: BTreeMap<&'static str, String>,
        on_click: Option<ClickFn>,
        on_input: Option<InputFn>,
        children: Vec<NodeId>,
    },
    Text(String),
}

#[derive(Default)]
struct Runtime {
    signals: Arena<SignalSlot>,
    effects: Arena<Effect>,
    scopes: Vec<Option<Scope>>,
    nodes: Vec<Option<Node>>,
    free_nodes: Vec<NodeId>,
    scope: ScopeId,
    tracking: Option<(u64, Handle)>,
    next_order: u64,
    dirty: BTreeSet<(u64, Handle)>,
    stats: UpdateStats,
    touched: HashSet<NodeId>,
    removed: usize,
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<RefCell<Runtime>>>> = const { RefCell::new(None) };
}

fn with_rt<R>(f: impl FnOnce(&mut Runtime) -> R) -> R {
    CURRENT.with(|c| {
        let rt = c.borrow().clone().expect("signal used outside an app");
        let mut rt = rt.borrow_mut();
        f(&mut rt)
    })
}

struct Enter;

impl Enter {
    fn new(rt: &Rc<RefCell<Runtime>>) -> Enter {
        CURRENT.with(|c| *c.borrow_mut() = Some(rt.clone()));
        Enter
    }
}

impl Drop for Enter {
    fn drop(&mut self) {
        CURRENT.with(|c| *c.borrow_mut() = None);
    }
}

impl Runtime {
    fn new_scope(&mut self, parent: Option<ScopeId>) -> ScopeId {
        self.scopes.push(Some(Scope { parent, ..Scope::default() }));
        let id = self.scopes.len() - 1;
        if let Some(p) = parent {
            self.scopes[p].as_mut().expect("live scope").children.push(id);
        }
        id
    }

    fn scope_mut(&mut self) -> &mut Scope {
        let s = self.scope;
        self.scopes[s].as_mut().expect("live scope")
    }

    fn new_node(&mut self, node: Node) -> NodeId {
        let id = if let Some(id) = self.free_nodes.pop() {
            self.nodes[id] = Some(node);
            id
        } else {
            self.nodes.push(Some(node));
            self.nodes.len() - 1
        };
        self.touched.insert(id);
        id
    }

    fn remove_node(&mut self, id: NodeId) {
        let node = self.nodes[id].take().expect("live node");
        self.touched.remove(&id);
        self.removed += 1;
        self.free_nodes.push(id);
        if let Node::Element { children, .. } = node {
            for c in children {
                self.remove_node(c);
            }
        }
    }

    fn unsubscribe(&mut self, effect: (u64, Handle)) {
        let deps = match self.effects.get_mut(effect.1) {
            Some(e) => std::mem::take(&mut e.deps),
            None => return,
        };
        for d in deps {
            if let Some(s) = self.signals.get_mut(d) {
                s.subscribers.remove(&effect);
            }
        }
    }

    fn dispose_scope(&mut self, id: ScopeId) {
        let scope = self.scopes[id].take().expect("live scope");
        if let Some(Some(parent)) = scope.parent.map(|p| self.scopes[p].as_mut()) {
            parent.children.retain(|&c| c != id);
        }
        for n in &scope.nodes {
            if self.nodes[*n].is_some() {
                self.remove_node(*n);
            }
        }
        for c in scope.children {
            self.dispose_scope(c);
        }
        for e in scope.effects {
            if let Some(order) = self.effects.get(e).map(|e| e.order) {
                self.unsubscribe((order, e));
                self.dirty.remove(&(order, e));
            }
            self.effects.remove(e);
        }
        for s in scope.signals {
            self.signals.remove(s);
        }
    }
}

/// Runs its closure when dropped, also when unwinding.
struct OnDrop<F: FnMut()>(F);

impl<F: FnMut()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        (self.0)()
    }
}

/// Runs `f` with no effect tracking reads, as `untrack` does in Leptos.
fn untracked<R>(f: impl FnOnce() -> R) -> R {
    let saved = with_rt(|rt| rt.tracking.take());
    let _restore = OnDrop(move || with_rt(|rt| rt.tracking = saved));
    f()
}

/// Runs `f` with `scope` owning what it creates.
fn in_scope<R>(scope: ScopeId, f: impl FnOnce() -> R) -> R {
    let saved = with_rt(|rt| std::mem::replace(&mut rt.scope, scope));
    let _restore = OnDrop(move || with_rt(|rt| rt.scope = saved));
    f()
}

fn run_effect(effect: (u64, Handle)) {
    let run = with_rt(|rt| {
        rt.unsubscribe(effect);
        rt.effects.get(effect.1).map(|e| e.run.clone())
    });
    let Some(run) = run else { return };
    let saved = with_rt(|rt| rt.tracking.replace(effect));
    let _restore = OnDrop(move || with_rt(|rt| rt.tracking = saved));
    run();
}

/// Creates an effect owned by the current scope and runs it once.
fn create_effect(f: impl Fn() + 'static) {
    let effect = with_rt(|rt| {
        let order = rt.next_order;
        rt.next_order += 1;
        let h = rt.effects.insert(Effect { run: Rc::new(f), deps: Vec::new(), order });
        rt.scope_mut().effects.push(h);
        (order, h)
    });
    run_effect(effect);
}

// ---- Signals ----------------------------------------------------------------

pub struct Signal<T> {
    handle: Handle,
    _t: PhantomData<fn() -> T>,
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Signal<T> {}

/// Creates a signal owned by the current scope.
pub fn signal<T: 'static>(value: T) -> Signal<T> {
    let handle = with_rt(|rt| {
        let h = rt.signals.insert(SignalSlot { value: Box::new(value), subscribers: BTreeSet::new() });
        rt.scope_mut().signals.push(h);
        h
    });
    Signal { handle, _t: PhantomData }
}

/// Puts a leased signal value back into its slot, also when the reader panics.
struct Lease {
    handle: Handle,
    value: Option<Box<dyn Any>>,
}

impl Drop for Lease {
    fn drop(&mut self) {
        let value = self.value.take().expect("leased value");
        with_rt(|rt| rt.signals.get_mut(self.handle).expect("live signal").value = value);
    }
}

/// Placeholder left in a slot while its value is leased.
struct Leased;

impl<T: 'static> Signal<T> {
    fn lease<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let value = with_rt(|rt| {
            let slot = rt.signals.get_mut(self.handle).expect("signal used after its scope was disposed");
            std::mem::replace(&mut slot.value, Box::new(Leased))
        });
        let mut lease = Lease { handle: self.handle, value: Some(value) };
        let value = lease.value.as_mut().unwrap();
        f(value.downcast_mut().expect("signal accessed again while it is in use"))
    }

    fn track(&self) {
        with_rt(|rt| {
            if let Some(effect) = rt.tracking {
                rt.signals.get_mut(self.handle).expect("live signal").subscribers.insert(effect);
                if let Some(e) = rt.effects.get_mut(effect.1) {
                    e.deps.push(self.handle);
                }
            }
        })
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.track();
        self.lease(|v| f(v))
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) {
        self.lease(f);
        with_rt(|rt| {
            let subs = rt.signals.get(self.handle).expect("live signal").subscribers.clone();
            rt.dirty.extend(subs);
        })
    }

    pub fn set(&self, value: T) {
        self.update(|v| *v = value)
    }
}

impl<T: Clone + 'static> Signal<T> {
    pub fn get(&self) -> T {
        self.with(T::clone)
    }
}

// ---- Views ------------------------------------------------------------------

type EachMount = Box<dyn FnOnce(NodeId)>;

pub enum View {
    Host(HostEl),
    Text(String),
    TextFn(StringFn),
}

pub struct HostEl {
    tag: &'static str,
    attrs: BTreeMap<&'static str, String>,
    attr_fns: Vec<(&'static str, StringFn)>,
    on_click: Option<ClickFn>,
    on_input: Option<InputFn>,
    children: Vec<View>,
    each: Option<EachMount>,
}

pub fn el(tag: &'static str) -> HostEl {
    HostEl {
        tag,
        attrs: BTreeMap::new(),
        attr_fns: Vec::new(),
        on_click: None,
        on_input: None,
        children: Vec::new(),
        each: None,
    }
}

pub fn text(s: impl Into<String>) -> View {
    View::Text(s.into())
}

/// A text node kept equal to `f()` by an effect.
pub fn text_fn(f: impl Fn() -> String + 'static) -> View {
    View::TextFn(Rc::new(f))
}

impl HostEl {
    pub fn id(self, id: impl Into<String>) -> Self {
        self.attr("id", id)
    }
    pub fn attr(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.attrs.insert(name, value.into());
        self
    }
    /// An attribute kept equal to `f()` by an effect.
    pub fn attr_fn(mut self, name: &'static str, f: impl Fn() -> String + 'static) -> Self {
        self.attr_fns.push((name, Rc::new(f)));
        self
    }
    pub fn child(mut self, child: impl Into<View>) -> Self {
        self.children.push(child.into());
        self
    }
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
    pub fn on_input(mut self, f: impl Fn(&str) + 'static) -> Self {
        self.on_input = Some(Rc::new(f));
        self
    }
    /// Makes this element's children one row per item of `items()`, keyed by
    /// `key`, each built once by `row` in a scope of its own.
    pub fn each<T, K>(
        mut self,
        items: impl Fn() -> Vec<T> + 'static,
        key: impl Fn(&T) -> K + 'static,
        row: impl Fn(T) -> View + 'static,
    ) -> Self
    where
        T: 'static,
        K: Eq + Hash + Clone + std::fmt::Debug + 'static,
    {
        self.each = Some(Box::new(move |parent| mount_each(parent, items, key, row)));
        self
    }
}

impl From<HostEl> for View {
    fn from(h: HostEl) -> View {
        View::Host(h)
    }
}

fn mount(view: View) -> NodeId {
    match view {
        View::Text(s) => with_rt(|rt| rt.new_node(Node::Text(s))),
        View::TextFn(f) => {
            let id = with_rt(|rt| rt.new_node(Node::Text(String::new())));
            create_effect(move || {
                let s = f();
                with_rt(|rt| {
                    if let Some(Node::Text(t)) = &mut rt.nodes[id]
                        && *t != s
                    {
                        *t = s;
                        rt.touched.insert(id);
                    }
                })
            });
            id
        }
        View::Host(h) => {
            let children: Vec<NodeId> = h.children.into_iter().map(mount).collect();
            let id = with_rt(|rt| {
                rt.new_node(Node::Element {
                    tag: h.tag,
                    attrs: h.attrs,
                    on_click: h.on_click,
                    on_input: h.on_input,
                    children,
                })
            });
            for (name, f) in h.attr_fns {
                create_effect(move || {
                    let v = f();
                    with_rt(|rt| {
                        if let Some(Node::Element { attrs, .. }) = &mut rt.nodes[id]
                            && attrs.get(name) != Some(&v)
                        {
                            attrs.insert(name, v);
                            rt.touched.insert(id);
                        }
                    })
                });
            }
            if let Some(each) = h.each {
                assert!(
                    with_rt(|rt| matches!(&rt.nodes[id], Some(Node::Element { children, .. }) if children.is_empty())),
                    "an element with each cannot also have static children"
                );
                each(id);
            }
            id
        }
    }
}

struct Row<K> {
    key: K,
    scope: ScopeId,
    node: NodeId,
}

/// Keeps one row per item, matched by key. A kept row is not rebuilt, even if
/// its item changed, as Leptos's `For` does. A kept row counts as moved under
/// React's `placeChild` rule.
fn mount_each<T, K>(
    parent: NodeId,
    items: impl Fn() -> Vec<T> + 'static,
    key: impl Fn(&T) -> K + 'static,
    row: impl Fn(T) -> View + 'static,
) where
    T: 'static,
    K: Eq + Hash + Clone + std::fmt::Debug + 'static,
{
    let owner = with_rt(|rt| rt.scope);
    let rows: Rc<RefCell<Vec<Row<K>>>> = Rc::new(RefCell::new(Vec::new()));
    create_effect(move || {
        let new_items = items();
        untracked(|| {
            let mut seen = HashSet::new();
            for item in &new_items {
                let k = key(item);
                assert!(seen.insert(k.clone()), "duplicate key {k:?} among rows");
            }
            let mut old: HashMap<K, (usize, Row<K>)> = std::mem::take(&mut *rows.borrow_mut())
                .into_iter()
                .enumerate()
                .map(|(i, r)| (r.key.clone(), (i, r)))
                .collect();
            let mut last_placed = 0;
            let mut out = Vec::with_capacity(new_items.len());
            with_rt(|rt| rt.stats.elements_diffed += new_items.len());
            for item in new_items {
                let k = key(&item);
                if let Some((old_index, kept)) = old.remove(&k) {
                    if old_index < last_placed {
                        with_rt(|rt| rt.touched.insert(kept.node));
                    } else {
                        last_placed = old_index;
                    }
                    out.push(kept);
                    continue;
                }
                let scope = with_rt(|rt| {
                    rt.stats.reruns += 1;
                    rt.new_scope(Some(owner))
                });
                let node = in_scope(scope, || mount(row(item)));
                with_rt(|rt| rt.scopes[scope].as_mut().expect("live scope").nodes.push(node));
                out.push(Row { key: k, scope, node });
            }
            for (_, (_, gone)) in old {
                with_rt(|rt| rt.dispose_scope(gone.scope));
            }
            with_rt(|rt| {
                if let Some(Node::Element { children, .. }) = &mut rt.nodes[parent] {
                    *children = out.iter().map(|r| r.node).collect();
                }
            });
            *rows.borrow_mut() = out;
        })
    });
}

// ---- App --------------------------------------------------------------------

/// What one update cost: effects re-run plus rows built, output nodes created,
/// removed, moved or changed, and list keys compared.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UpdateStats {
    pub reruns: usize,
    pub nodes_changed: usize,
    pub elements_diffed: usize,
}

pub struct App {
    rt: Rc<RefCell<Runtime>>,
    root: NodeId,
    stats: UpdateStats,
}

impl App {
    pub fn new(root: impl FnOnce() -> View) -> App {
        let rt = Rc::new(RefCell::new(Runtime::default()));
        let _enter = Enter::new(&rt);
        let scope = with_rt(|rt| rt.new_scope(None));
        with_rt(|rt| rt.scope = scope);
        let root = mount(root());
        with_rt(|rt| rt.dirty.clear());
        drop(_enter);
        App { rt, root, stats: UpdateStats::default() }
    }

    pub fn click(&mut self, id: &str) {
        let handler = match self.node(id) {
            Node::Element { on_click: Some(f), .. } => f.clone(),
            _ => panic!("no click handler on #{id}"),
        };
        self.dispatch(|| handler());
    }

    pub fn input(&mut self, id: &str, value: &str) {
        let handler = match self.node(id) {
            Node::Element { on_input: Some(f), .. } => f.clone(),
            _ => panic!("no input handler on #{id}"),
        };
        self.dispatch(|| handler(value));
    }

    pub fn last_update(&self) -> UpdateStats {
        self.stats
    }

    /// Concatenated text under the node with this id.
    pub fn text(&self, id: &str) -> String {
        let rt = self.rt.borrow();
        let mut out = String::new();
        walk(&rt, find(&rt, self.root, id).unwrap_or_else(|| panic!("no node with id #{id}")), &mut |n| {
            if let Node::Text(s) = n {
                out.push_str(s);
            }
        });
        out
    }

    pub fn attr(&self, id: &str, name: &str) -> Option<String> {
        match self.node(id) {
            Node::Element { attrs, .. } => attrs.get(name).cloned(),
            Node::Text(_) => None,
        }
    }

    /// Every `id` attribute in document order.
    pub fn ids(&self) -> Vec<String> {
        let rt = self.rt.borrow();
        let mut out = Vec::new();
        walk(&rt, self.root, &mut |n| {
            if let Node::Element { attrs, .. } = n
                && let Some(id) = attrs.get("id")
            {
                out.push(id.clone());
            }
        });
        out
    }

    fn node(&self, id: &str) -> Node {
        let rt = self.rt.borrow();
        let n = find(&rt, self.root, id).unwrap_or_else(|| panic!("no node with id #{id}"));
        match rt.nodes[n].as_ref().unwrap() {
            Node::Element { tag, attrs, on_click, on_input, .. } => Node::Element {
                tag,
                attrs: attrs.clone(),
                on_click: on_click.clone(),
                on_input: on_input.clone(),
                children: Vec::new(),
            },
            Node::Text(s) => Node::Text(s.clone()),
        }
    }

    fn dispatch(&mut self, f: impl FnOnce()) {
        let _enter = Enter::new(&self.rt);
        with_rt(|rt| {
            rt.stats = UpdateStats::default();
            rt.touched.clear();
            rt.removed = 0;
        });
        f();
        while let Some(next) = with_rt(|rt| rt.dirty.pop_first()) {
            with_rt(|rt| rt.stats.reruns += 1);
            run_effect(next);
        }
        self.stats = with_rt(|rt| {
            let mut s = rt.stats;
            s.nodes_changed = rt.touched.len() + rt.removed;
            s
        });
    }
}

fn walk(rt: &Runtime, id: NodeId, f: &mut dyn FnMut(&Node)) {
    let node = rt.nodes[id].as_ref().expect("live node");
    f(node);
    if let Node::Element { children, .. } = node {
        for &c in children {
            walk(rt, c, f);
        }
    }
}

fn find(rt: &Runtime, at: NodeId, id: &str) -> Option<NodeId> {
    match rt.nodes[at].as_ref()? {
        Node::Element { attrs, children, .. } => {
            if attrs.get("id").map(String::as_str) == Some(id) {
                return Some(at);
            }
            children.iter().find_map(|&c| find(rt, c, id))
        }
        Node::Text(_) => None,
    }
}

