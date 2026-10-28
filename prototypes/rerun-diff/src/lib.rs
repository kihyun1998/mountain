//! Headless prototype of the re-run-and-diff state model.
//!
//! A component's `render` re-runs when its state changes or its props differ,
//! and its output is diffed against the instance tree kept from the last run.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::marker::PhantomData;
use std::rc::Rc;

pub type Key = String;
type InstId = usize;
type ClickFn = Rc<dyn Fn()>;
type InputFn = Rc<dyn Fn(&str)>;

// ---- Components and the element description -------------------------------

pub trait Component: PartialEq + 'static {
    fn render(&self, cx: &mut Cx) -> El;
}

trait AnyComponent {
    fn type_of(&self) -> TypeId;
    fn as_any(&self) -> &dyn Any;
    fn eq_dyn(&self, other: &dyn AnyComponent) -> bool;
    fn render_dyn(&self, cx: &mut Cx) -> El;
}

impl<C: Component> AnyComponent for C {
    fn type_of(&self) -> TypeId {
        TypeId::of::<C>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_dyn(&self, other: &dyn AnyComponent) -> bool {
        other.as_any().downcast_ref::<C>() == Some(self)
    }
    fn render_dyn(&self, cx: &mut Cx) -> El {
        self.render(cx)
    }
}

pub enum El {
    Host(HostEl),
    Text(String),
    Comp(CompEl),
}

pub struct HostEl {
    tag: &'static str,
    key: Option<Key>,
    attrs: BTreeMap<&'static str, String>,
    on_click: Option<ClickFn>,
    on_input: Option<InputFn>,
    children: Vec<El>,
}

pub struct CompEl {
    key: Option<Key>,
    comp: Box<dyn AnyComponent>,
}

pub fn el(tag: &'static str) -> HostEl {
    HostEl {
        tag,
        key: None,
        attrs: BTreeMap::new(),
        on_click: None,
        on_input: None,
        children: Vec::new(),
    }
}

pub fn text(s: impl Into<String>) -> El {
    El::Text(s.into())
}

pub fn comp<C: Component>(c: C) -> El {
    El::Comp(CompEl { key: None, comp: Box::new(c) })
}

pub fn comp_keyed<C: Component>(key: impl Into<Key>, c: C) -> El {
    El::Comp(CompEl { key: Some(key.into()), comp: Box::new(c) })
}

impl HostEl {
    pub fn id(self, id: impl Into<String>) -> Self {
        self.attr("id", id)
    }
    pub fn key(mut self, key: impl Into<Key>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn attr(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.attrs.insert(name, value.into());
        self
    }
    pub fn child(mut self, child: impl Into<El>) -> Self {
        self.children.push(child.into());
        self
    }
    pub fn children(mut self, children: impl IntoIterator<Item = El>) -> Self {
        self.children.extend(children);
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
}

impl From<HostEl> for El {
    fn from(h: HostEl) -> El {
        El::Host(h)
    }
}

impl El {
    fn key(&self) -> Option<&Key> {
        match self {
            El::Host(h) => h.key.as_ref(),
            El::Comp(c) => c.key.as_ref(),
            El::Text(_) => None,
        }
    }
}

// ---- State: copyable handles into a per-app slot store --------------------

struct Slot {
    value: Box<dyn Any>,
    owner: InstId,
}

#[derive(Default)]
struct Runtime {
    slots: Vec<Option<Slot>>,
    gens: Vec<u32>,
    free: Vec<usize>,
    dirty: Vec<InstId>,
    rendering: bool,
}

impl Runtime {
    fn alloc(&mut self, value: Box<dyn Any>, owner: InstId) -> usize {
        let slot = Some(Slot { value, owner });
        if let Some(i) = self.free.pop() {
            self.slots[i] = slot;
            i
        } else {
            self.slots.push(slot);
            self.gens.push(0);
            self.slots.len() - 1
        }
    }
    fn release(&mut self, i: usize) {
        self.slots[i] = None;
        self.gens[i] += 1;
        self.free.push(i);
    }
    fn slot(&mut self, i: usize, generation: u32) -> &mut Slot {
        assert_eq!(self.gens[i], generation, "state used after its component unmounted");
        self.slots[i].as_mut().expect("live slot")
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<RefCell<Runtime>>>> = const { RefCell::new(None) };
}

fn with_rt<R>(f: impl FnOnce(&mut Runtime) -> R) -> R {
    CURRENT.with(|c| {
        let rt = c.borrow().clone().expect("state used outside an app");
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

pub struct State<T> {
    slot: usize,
    generation: u32,
    _t: PhantomData<fn() -> T>,
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for State<T> {}

/// Puts a leased state value back into its slot, also when the reader panics.
struct Lease {
    slot: usize,
    generation: u32,
    value: Option<Box<dyn Any>>,
}

impl Drop for Lease {
    fn drop(&mut self) {
        let value = self.value.take().expect("leased value");
        with_rt(|rt| rt.slot(self.slot, self.generation).value = value);
    }
}

/// Placeholder left in a slot while its value is leased.
struct Leased;

impl<T: 'static> State<T> {
    /// Runs `f` with the value moved out of the store, so `f` may read other
    /// states.
    fn lease<R>(&self, f: impl FnOnce(&mut T) -> R) -> (R, InstId) {
        let (value, owner) = with_rt(|rt| {
            let slot = rt.slot(self.slot, self.generation);
            (std::mem::replace(&mut slot.value, Box::new(Leased)), slot.owner)
        });
        let mut lease = Lease { slot: self.slot, generation: self.generation, value: Some(value) };
        let value = lease.value.as_mut().unwrap();
        let out = f(value.downcast_mut().expect("state accessed again while it is in use"));
        (out, owner)
    }
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.lease(|v| f(v)).0
    }
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        let ((), owner) = self.lease(f);
        with_rt(|rt| rt.dirty.push(owner));
    }
    pub fn set(&self, value: T) {
        self.update(|v| *v = value)
    }
    /// A write-only handle that can be passed to a child as a prop.
    pub fn setter(&self) -> Setter<T> {
        Setter { state: *self }
    }
}

/// Write access to a state, comparable so it can sit in a child's props.
/// Children read parent state through value props, never through a handle.
pub struct Setter<T> {
    state: State<T>,
}

impl<T> Clone for Setter<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Setter<T> {}
impl<T> PartialEq for Setter<T> {
    fn eq(&self, other: &Self) -> bool {
        self.state.slot == other.state.slot && self.state.generation == other.state.generation
    }
}

impl<T: 'static> Setter<T> {
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        let rendering = with_rt(|rt| rt.rendering);
        assert!(!rendering, "cannot update another component's state while rendering");
        self.state.update(f)
    }
    pub fn set(&self, value: T) {
        self.update(|v| *v = value)
    }
}

impl<T: Clone + 'static> State<T> {
    pub fn get(&self) -> T {
        self.with(T::clone)
    }
}

#[derive(Clone, Copy)]
struct Hook {
    slot: usize,
    generation: u32,
    ty: TypeId,
}

pub struct Cx {
    owner: InstId,
    hooks: Vec<Hook>,
    next: usize,
    first_run: bool,
}

impl Cx {
    pub fn use_state<T: 'static>(&mut self, init: impl FnOnce() -> T) -> State<T> {
        if self.next == self.hooks.len() {
            assert!(self.first_run, "hook order changed: more hooks than the previous render");
            let owner = self.owner;
            let slot = with_rt(|rt| rt.alloc(Box::new(init()), owner));
            let generation = with_rt(|rt| rt.gens[slot]);
            self.hooks.push(Hook { slot, generation, ty: TypeId::of::<T>() });
        }
        let hook = self.hooks[self.next];
        assert!(hook.ty == TypeId::of::<T>(), "hook order changed: hook {} has another type", self.next);
        self.next += 1;
        State { slot: hook.slot, generation: hook.generation, _t: PhantomData }
    }

    fn finish(&self) {
        assert!(
            self.first_run || self.next == self.hooks.len(),
            "hook order changed: fewer hooks than the previous render"
        );
    }
}

// ---- Instances and reconciliation ------------------------------------------

enum Kind {
    Host {
        tag: &'static str,
        attrs: BTreeMap<&'static str, String>,
        on_click: Option<ClickFn>,
        on_input: Option<InputFn>,
        children: Vec<InstId>,
    },
    Text(String),
    Comp {
        comp: Box<dyn AnyComponent>,
        hooks: Vec<Hook>,
        child: Option<InstId>,
    },
}

struct Instance {
    key: Option<Key>,
    depth: usize,
    kind: Kind,
}

/// What one update cost: components whose `render` ran, output nodes
/// created, removed, moved or changed, and elements compared by the diff.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UpdateStats {
    pub reruns: usize,
    pub nodes_changed: usize,
    pub elements_diffed: usize,
}

pub struct App {
    rt: Rc<RefCell<Runtime>>,
    insts: Vec<Option<Instance>>,
    free: Vec<InstId>,
    root: InstId,
    dirty: HashSet<InstId>,
    touched: HashSet<InstId>,
    removed: usize,
    stats: UpdateStats,
}

impl App {
    pub fn new<C: Component>(root: C) -> App {
        let rt = Rc::new(RefCell::new(Runtime::default()));
        let mut app = App {
            rt,
            insts: Vec::new(),
            free: Vec::new(),
            root: 0,
            dirty: HashSet::new(),
            touched: HashSet::new(),
            removed: 0,
            stats: UpdateStats::default(),
        };
        let _enter = Enter::new(&app.rt.clone());
        app.root = app.mount(comp(root), 0);
        app.rt.borrow_mut().dirty.clear();
        app
    }

    pub fn click(&mut self, id: &str) {
        let handler = match &self.find(id).kind {
            Kind::Host { on_click: Some(f), .. } => f.clone(),
            _ => panic!("no click handler on #{id}"),
        };
        self.dispatch(|| handler());
    }

    pub fn input(&mut self, id: &str, value: &str) {
        let handler = match &self.find(id).kind {
            Kind::Host { on_input: Some(f), .. } => f.clone(),
            _ => panic!("no input handler on #{id}"),
        };
        self.dispatch(|| handler(value));
    }

    pub fn last_update(&self) -> UpdateStats {
        self.stats
    }

    /// Concatenated text under the node with this id.
    pub fn text(&self, id: &str) -> String {
        let mut out = String::new();
        self.collect_text(self.find_id(id), &mut out);
        out
    }

    pub fn attr(&self, id: &str, name: &str) -> Option<String> {
        match &self.find(id).kind {
            Kind::Host { attrs, .. } => attrs.get(name).cloned(),
            _ => None,
        }
    }

    /// Every `id` attribute in document order.
    pub fn ids(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.walk(self.root, &mut |inst| {
            if let Kind::Host { attrs, .. } = &inst.kind
                && let Some(id) = attrs.get("id")
            {
                out.push(id.clone());
            }
        });
        out
    }

    fn dispatch(&mut self, f: impl FnOnce()) {
        let _enter = Enter::new(&self.rt.clone());
        self.stats = UpdateStats::default();
        self.touched.clear();
        self.removed = 0;
        f();
        self.flush();
        self.stats.nodes_changed = self.touched.len() + self.removed;
    }

    fn flush(&mut self) {
        loop {
            let pending: Vec<InstId> = self.rt.borrow_mut().dirty.drain(..).collect();
            self.dirty.extend(pending);
            let Some(&next) = self
                .dirty
                .iter()
                .min_by_key(|&&id| self.insts[id].as_ref().map_or(0, |i| i.depth))
            else {
                break;
            };
            self.dirty.remove(&next);
            if self.insts[next].is_some() {
                self.render_comp(next);
            }
        }
    }

    fn alloc(&mut self, inst: Instance) -> InstId {
        if let Some(id) = self.free.pop() {
            self.insts[id] = Some(inst);
            id
        } else {
            self.insts.push(Some(inst));
            self.insts.len() - 1
        }
    }

    fn mount(&mut self, el: El, depth: usize) -> InstId {
        match el {
            El::Text(s) => {
                let id = self.alloc(Instance { key: None, depth, kind: Kind::Text(s) });
                self.touched.insert(id);
                id
            }
            El::Host(h) => {
                check_unique_keys(&h.children);
                let children = h.children.into_iter().map(|c| self.mount(c, depth + 1)).collect();
                let id = self.alloc(Instance {
                    key: h.key,
                    depth,
                    kind: Kind::Host {
                        tag: h.tag,
                        attrs: h.attrs,
                        on_click: h.on_click,
                        on_input: h.on_input,
                        children,
                    },
                });
                self.touched.insert(id);
                id
            }
            El::Comp(c) => {
                let id = self.alloc(Instance {
                    key: c.key,
                    depth,
                    kind: Kind::Comp { comp: c.comp, hooks: Vec::new(), child: None },
                });
                self.render_comp(id);
                id
            }
        }
    }

    fn render_comp(&mut self, id: InstId) {
        self.stats.reruns += 1;
        self.dirty.remove(&id);
        let mut inst = self.insts[id].take().expect("live instance");
        let Kind::Comp { comp, hooks, child } = &mut inst.kind else {
            unreachable!("render_comp on a non-component")
        };
        let first_run = child.is_none();
        let mut cx = Cx { owner: id, hooks: std::mem::take(hooks), next: 0, first_run };
        let was_rendering = std::mem::replace(&mut self.rt.borrow_mut().rendering, true);
        let out = comp.render_dyn(&mut cx);
        self.rt.borrow_mut().rendering = was_rendering;
        cx.finish();
        *hooks = cx.hooks;
        let old_child = child.take();
        let depth = inst.depth;
        self.insts[id] = Some(inst);
        let new_child = match old_child {
            Some(c) => self.reconcile(c, out, depth + 1),
            None => self.mount(out, depth + 1),
        };
        if let Some(Instance { kind: Kind::Comp { child, .. }, .. }) = &mut self.insts[id] {
            *child = Some(new_child);
        }
    }

    fn reconcile(&mut self, old: InstId, new: El, depth: usize) -> InstId {
        self.stats.elements_diffed += 1;
        let inst = self.insts[old].as_mut().expect("live instance");
        if inst.key.as_ref() != new.key() {
            self.unmount(old);
            return self.mount(new, depth);
        }
        match (&mut inst.kind, new) {
            (Kind::Text(s), El::Text(t)) => {
                if *s != t {
                    *s = t;
                    self.touched.insert(old);
                }
                old
            }
            (Kind::Host { tag, attrs, on_click, on_input, children }, El::Host(h)) if *tag == h.tag => {
                if *attrs != h.attrs {
                    *attrs = h.attrs;
                    self.touched.insert(old);
                }
                *on_click = h.on_click;
                *on_input = h.on_input;
                let old_children = std::mem::take(children);
                let new_children = self.reconcile_children(old_children, h.children, depth + 1);
                if let Some(Instance { kind: Kind::Host { children, .. }, .. }) = &mut self.insts[old] {
                    *children = new_children;
                }
                old
            }
            (Kind::Comp { comp, .. }, El::Comp(c)) if comp.type_of() == c.comp.type_of() => {
                if comp.eq_dyn(&*c.comp) && !self.dirty.contains(&old) {
                    return old;
                }
                *comp = c.comp;
                self.render_comp(old);
                old
            }
            (_, new) => {
                self.unmount(old);
                self.mount(new, depth)
            }
        }
    }

    /// Matches new children to old ones by key, and unkeyed ones by position
    /// among the unkeyed. A kept child counts as moved under React's
    /// `placeChild` rule: its old index is below the last index left in place.
    fn reconcile_children(&mut self, old: Vec<InstId>, new: Vec<El>, depth: usize) -> Vec<InstId> {
        let mut keyed: HashMap<Key, (usize, InstId)> = HashMap::new();
        let mut unkeyed: VecDeque<(usize, InstId)> = VecDeque::new();
        for (index, id) in old.into_iter().enumerate() {
            match self.insts[id].as_ref().and_then(|i| i.key.clone()) {
                Some(k) => {
                    keyed.insert(k, (index, id));
                }
                None => unkeyed.push_back((index, id)),
            }
        }
        check_unique_keys(&new);
        let mut last_placed = 0;
        let mut out = Vec::with_capacity(new.len());
        for el in new {
            let matched = match el.key() {
                Some(k) => keyed.remove(k),
                None => unkeyed.pop_front(),
            };
            let id = match matched {
                Some((old_index, o)) => {
                    let id = self.reconcile(o, el, depth);
                    if id == o {
                        if old_index < last_placed {
                            self.touched.insert(id);
                        } else {
                            last_placed = old_index;
                        }
                    }
                    id
                }
                None => self.mount(el, depth),
            };
            out.push(id);
        }
        for (_, o) in keyed.into_values().chain(unkeyed) {
            self.unmount(o);
        }
        out
    }

    fn unmount(&mut self, id: InstId) {
        let inst = self.insts[id].take().expect("live instance");
        self.dirty.remove(&id);
        self.touched.remove(&id);
        match inst.kind {
            Kind::Text(_) => self.removed += 1,
            Kind::Host { children, .. } => {
                self.removed += 1;
                for c in children {
                    self.unmount(c);
                }
            }
            Kind::Comp { hooks, child, .. } => {
                let mut rt = self.rt.borrow_mut();
                for hook in hooks {
                    rt.release(hook.slot);
                }
                drop(rt);
                if let Some(c) = child {
                    self.unmount(c);
                }
            }
        }
        self.free.push(id);
    }

    fn walk(&self, id: InstId, f: &mut dyn FnMut(&Instance)) {
        let inst = self.insts[id].as_ref().expect("live instance");
        f(inst);
        match &inst.kind {
            Kind::Host { children, .. } => {
                for &c in children {
                    self.walk(c, f);
                }
            }
            Kind::Comp { child: Some(c), .. } => self.walk(*c, f),
            _ => {}
        }
    }

    fn find_id(&self, id: &str) -> InstId {
        fn go(app: &App, at: InstId, id: &str) -> Option<InstId> {
            let inst = app.insts[at].as_ref()?;
            match &inst.kind {
                Kind::Host { attrs, children, .. } => {
                    if attrs.get("id").map(String::as_str) == Some(id) {
                        return Some(at);
                    }
                    children.iter().find_map(|&c| go(app, c, id))
                }
                Kind::Comp { child: Some(c), .. } => go(app, *c, id),
                _ => None,
            }
        }
        go(self, self.root, id).unwrap_or_else(|| panic!("no node with id #{id}"))
    }

    fn find(&self, id: &str) -> &Instance {
        self.insts[self.find_id(id)].as_ref().unwrap()
    }

    fn collect_text(&self, id: InstId, out: &mut String) {
        self.walk(id, &mut |inst| {
            if let Kind::Text(s) = &inst.kind {
                out.push_str(s);
            }
        });
    }
}

fn check_unique_keys(children: &[El]) {
    let mut seen = HashSet::new();
    for k in children.iter().filter_map(El::key) {
        assert!(seen.insert(k), "duplicate key {k:?} among siblings");
    }
}
