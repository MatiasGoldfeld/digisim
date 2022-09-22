use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};

use crate::circuit::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(u64);

impl NodeId {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::SeqCst))
    }
}

impl Into<usize> for NodeId {
    fn into(self) -> usize {
        self.0 as usize
    }
}

pub type Nodes = HashMap<NodeId, Box<dyn Node>>;

#[derive(Debug)]
pub struct Scheduler {
    tick: Tick,
    next: HashSet<NodeId>,
    queue: HashMap<Tick, HashSet<NodeId>>,
    changed: HashSet<NodeId>,
}

// TODO: Consider making [Node] an enum instead of a trait
pub trait Node: Debug + Send + Sync {
    fn id(&self) -> NodeId;
    fn add_input(&mut self, node_id: NodeId, input_active: Arc<AtomicBool>);
    fn add_output(&mut self, node_id: NodeId);
    fn update(&self, scheduler: &mut Scheduler, nodes: &Nodes);
    fn apply_change(&self);
    fn trigger(&self, _scheduler: &mut Scheduler, _new_active: bool) {
        // TODO: this is gross pls remove
        panic!("Not a trigger!")
    }
    fn get_active(&self) -> Arc<AtomicBool>;
}

impl Scheduler {
    pub fn new() -> Self {
        Scheduler {
            tick: 0,
            next: HashSet::new(),
            queue: HashMap::new(),
            changed: HashSet::new(),
        }
    }

    pub fn enqueue_next(&mut self, node_id: NodeId) {
        self.next.insert(node_id);
    }

    pub fn enqueue(&mut self, after: Ticks, node_id: NodeId) {
        assert!(after > 0);
        // TODO: Perform some sort of HashSet bucket keeping technique
        self.queue
            .entry(self.tick + after)
            .or_default()
            .insert(node_id);
    }

    pub fn enqueue_changed(&mut self, node_id: NodeId) {
        self.changed.insert(node_id);
    }

    fn drain_changed(&mut self, nodes: &Nodes) {
        self.changed
            .drain()
            .for_each(|node_id| nodes.get(&node_id).unwrap().apply_change());
    }

    pub fn update(&mut self, nodes: &Nodes) {
        // println!("Scheduler update (tick {})", self.tick);
        // TODO: Perhaps merge sets before updating their nodes?
        self.drain_changed(nodes);
        self.next
            .drain()
            .collect::<Vec<_>>()
            .into_iter()
            .for_each(|node_id| nodes.get(&node_id).unwrap().update(self, nodes));
        match self.queue.remove(&self.tick) {
            Some(node_ids) => node_ids
                .into_iter()
                .for_each(|node_id| nodes.get(&node_id).unwrap().update(self, nodes)),
            None => (),
        };
        self.drain_changed(nodes);
        self.tick = self.tick + 1;
    }

    pub fn is_empty(&self) -> bool {
        self.next.is_empty() && self.queue.is_empty()
    }
}

#[derive(Debug)]
struct Wire {
    id: NodeId,
    inputs: HashMap<NodeId, Arc<AtomicBool>>,
    outputs: HashSet<NodeId>,
    active: Arc<AtomicBool>,
}

impl Wire {
    pub fn new() -> Self {
        Self {
            id: NodeId::new(),
            inputs: Default::default(),
            outputs: Default::default(),
            active: Default::default(),
        }
    }
}

pub trait RelaxedAtomic {
    fn get(&self) -> bool;
    fn set(&self, val: bool);
}

impl RelaxedAtomic for AtomicBool {
    fn get(&self) -> bool {
        self.load(Ordering::Relaxed)
    }

    fn set(&self, val: bool) {
        self.store(val, Ordering::Relaxed)
    }
}

impl Node for Wire {
    fn id(&self) -> NodeId {
        self.id
    }

    fn add_input(&mut self, node_id: NodeId, input_active: Arc<AtomicBool>) {
        self.inputs.insert(node_id, input_active);
    }

    fn add_output(&mut self, node_id: NodeId) {
        self.outputs.insert(node_id);
    }

    fn update(&self, scheduler: &mut Scheduler, nodes: &Nodes) {
        let new_active = self.inputs.values().any(|input| input.get());
        if new_active != self.active.get() {
            self.active.set(new_active);
            // TODO: Schedule output updates as to not potentially do them twice
            self.outputs
                .iter()
                .for_each(|output| nodes.get(output).unwrap().update(scheduler, nodes))
        };
    }

    fn apply_change(&self) {}

    fn get_active(&self) -> Arc<AtomicBool> {
        self.active.clone()
    }
}

// TODO: Perhaps have some 0-node_id stub node which is always is not active
// and does not update
#[derive(Debug)]
struct Inverter {
    id: NodeId,
    input: Option<(NodeId, Arc<AtomicBool>)>,
    output: Option<NodeId>,
    active: Arc<AtomicBool>,
    next_active: AtomicBool,
}

impl Inverter {
    pub fn new(input: Option<(NodeId, Arc<AtomicBool>)>, output: Option<NodeId>) -> Self {
        Self {
            id: NodeId::new(),
            input,
            output,
            active: Arc::new(AtomicBool::new(true)),
            next_active: AtomicBool::new(true),
        }
    }
}

impl Node for Inverter {
    fn id(&self) -> NodeId {
        self.id
    }

    fn add_input(&mut self, node_id: NodeId, input_active: Arc<AtomicBool>) {
        match self.input {
            Some(_) => panic!("Inverter already has input"),
            None => self.input = Some((node_id, input_active)),
        }
    }

    fn add_output(&mut self, node_id: NodeId) {
        match self.output {
            Some(_) => panic!("Inverter already has output"),
            None => self.output = Some(node_id),
        }
    }

    fn update(&self, scheduler: &mut Scheduler, _nodes: &Nodes) {
        self.next_active.set(match &self.input {
            Some((_, input)) => !input.get(),
            None => true,
        });
        if self.next_active.get() != self.active.get() {
            scheduler.enqueue_changed(self.id);
            match self.output {
                Some(output) => scheduler.enqueue_next(output),
                None => (),
            }
        }
    }

    fn apply_change(&self) {
        self.active.set(self.next_active.get());
    }

    fn get_active(&self) -> Arc<AtomicBool> {
        self.active.clone()
    }
}

#[derive(Debug)]
pub struct Trigger {
    id: NodeId,
    output: Option<NodeId>,
    pub active: Arc<AtomicBool>,
    next_active: AtomicBool,
}

impl Trigger {
    pub fn new(output: Option<NodeId>) -> Self {
        Self {
            id: NodeId::new(),
            output,
            active: Arc::new(AtomicBool::new(false)),
            next_active: AtomicBool::new(false),
        }
    }
}

impl Node for Trigger {
    fn id(&self) -> NodeId {
        self.id
    }

    fn add_input(&mut self, _: NodeId, _: Arc<AtomicBool>) {
        panic!("Trigger has no inputs")
    }

    fn add_output(&mut self, node_id: NodeId) {
        match self.output {
            Some(_) => panic!("Trigger already has output"),
            None => self.output = Some(node_id),
        }
    }

    fn update(&self, _scheduler: &mut Scheduler, _nodes: &Nodes) {}

    fn apply_change(&self) {
        self.active.set(self.next_active.get());
    }

    fn trigger(&self, scheduler: &mut Scheduler, new_active: bool) {
        if new_active != self.next_active.get() {
            self.next_active.set(new_active);
            scheduler.enqueue_changed(self.id);
            match self.output {
                Some(node_id) => scheduler.enqueue_next(node_id),
                None => (),
            };
        }
    }

    fn get_active(&self) -> Arc<AtomicBool> {
        self.active.clone()
    }
}

#[derive(Debug)]
pub struct CircuitNaive {
    scheduler: Scheduler,
    nodes: Nodes,
}

impl Circuit for CircuitNaive {
    type NodeId = NodeId;

    fn new() -> Self {
        Self {
            scheduler: Scheduler::new(),
            nodes: HashMap::new(),
        }
    }

    fn tick(&self) -> Tick {
        self.scheduler.tick
    }

    fn update(&mut self) {
        self.scheduler.update(&self.nodes);
    }

    fn work_left(&self) -> bool {
        !self.scheduler.is_empty()
    }

    fn wire(&mut self) -> NodeId {
        let node = Box::new(Wire::new());
        let node_id = node.id;
        self.scheduler.enqueue_next(node_id);
        self.nodes.insert(node_id, node);
        node_id
    }

    fn inverter(&mut self) -> NodeId {
        let node = Box::new(Inverter::new(None, None));
        let node_id = node.id;
        self.scheduler.enqueue_next(node_id);
        self.nodes.insert(node_id, node);
        node_id
    }

    fn trigger(&mut self) -> NodeId {
        let node = Box::new(Trigger::new(None));
        let node_id = node.id;
        self.scheduler.enqueue_next(node_id);
        self.nodes.insert(node_id, node);
        node_id
    }

    fn connect(&mut self, input: NodeId, output: NodeId) {
        let input_node = self.nodes.get_mut(&input).unwrap();
        input_node.add_output(output);
        let input_active = input_node.get_active();
        self.nodes
            .get_mut(&output)
            .unwrap()
            .add_input(input, input_active);
    }

    fn trigger_node(&mut self, node_id: NodeId, val: bool) {
        self.nodes
            .get(&node_id)
            .unwrap()
            .trigger(&mut self.scheduler, val);
    }

    fn is_active(&self, node_id: NodeId) -> bool {
        self.nodes.get(&node_id).unwrap().get_active().get()
    }
}
