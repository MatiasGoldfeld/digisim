use std::borrow::Borrow;
use std::cell::{Cell, RefCell};
use std::collections::{hash_map::HashMap, HashSet};
use std::hash::Hash;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

type Tick = u64;
type Ticks = u64;
type Nodes = HashMap<NodeId, Box<dyn Node>>;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct NodeId(u64);

impl NodeId {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        NodeId(NEXT.fetch_add(1, Ordering::SeqCst))
    }
}

struct Scheduler {
    tick: Tick,
    next: HashSet<NodeId>,
    queue: HashMap<Tick, HashSet<NodeId>>,
    changed: HashSet<NodeId>,
}

// TODO: Consider making [Node] an enum instead of a trait
trait Node {
    fn id(&self) -> NodeId;
    fn add_output(&mut self, node_id: NodeId);
    fn update(&self, scheduler: &mut Scheduler, nodes: &Nodes);
    fn apply_change(&self);
    fn trigger(&self, _scheduler: &mut Scheduler, _new_active: bool) {
        // TODO: this is gross pls remove
        panic!("Not a trigger!")
    }
}

impl Scheduler {
    fn new() -> Self {
        Scheduler {
            tick: 0,
            next: HashSet::new(),
            queue: HashMap::new(),
            changed: HashSet::new(),
        }
    }

    fn enqueue_next(&mut self, node_id: NodeId) {
        self.next.insert(node_id);
    }

    fn enqueue(&mut self, after: Ticks, node_id: NodeId) {
        assert!(after > 0);
        // TODO: Perform some sort of HashSet bucket keeping technique
        self.queue
            .entry(self.tick + after)
            .or_default()
            .insert(node_id);
    }

    fn enqueue_changed(&mut self, node_id: NodeId) {
        self.changed.insert(node_id);
    }

    fn drain_changed(&mut self, nodes: &Nodes) {
        self.changed
            .drain()
            .for_each(|node_id| nodes.get(&node_id).unwrap().apply_change());
    }

    fn update(&mut self, nodes: &Nodes) {
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

    fn is_empty(&self) -> bool {
        self.next.is_empty() && self.queue.is_empty()
    }
}

struct Wire {
    id: NodeId,
    inputs: HashMap<NodeId, Rc<Cell<bool>>>,
    outputs: HashSet<NodeId>,
    active: Rc<Cell<bool>>,
}

impl Wire {
    fn new() -> Self {
        Self {
            id: NodeId::new(),
            inputs: Default::default(),
            outputs: Default::default(),
            active: Default::default(),
        }
    }
}

impl Node for Wire {
    fn id(&self) -> NodeId {
        self.id
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
}

// TODO: Perhaps have some 0-node_id stub node which is always is not active
// and does not update
struct Inverter {
    id: NodeId,
    input: Option<(NodeId, Rc<Cell<bool>>)>,
    output: Option<NodeId>,
    active: Rc<Cell<bool>>,
    next_active: Cell<bool>,
}

impl Node for Inverter {
    fn id(&self) -> NodeId {
        self.id
    }

    fn add_output(&mut self, node_id: NodeId) {
        match self.output {
            Some(_) => panic!("Inserter already has output"),
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
}

struct Trigger {
    id: NodeId,
    output: Option<NodeId>,
    active: Rc<Cell<bool>>,
    next_active: Cell<bool>,
}

impl Node for Trigger {
    fn id(&self) -> NodeId {
        self.id
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
}

struct Circuit {
    scheduler: Scheduler,
    nodes: Nodes,
}

#[derive(Debug)]
enum RunResult {
    Finished { after_ticks: Ticks },
    ReachedMaxTicks { max_ticks: Ticks },
}

impl Circuit {
    pub fn new() -> Self {
        Circuit {
            scheduler: Scheduler::new(),
            nodes: HashMap::new(),
        }
    }

    pub fn run(&mut self, max_ticks: Ticks) -> RunResult {
        for ticks in 0..max_ticks {
            if self.scheduler.is_empty() {
                return RunResult::Finished { after_ticks: ticks };
            } else {
                self.scheduler.update(&self.nodes)
            };
        }
        RunResult::ReachedMaxTicks { max_ticks }
    }

    fn add_node(&mut self, node: Box<dyn Node>) {
        let node_id = node.id();
        match self.nodes.insert(node_id, node) {
            // TODO: Make this nicer
            Some(_) => panic!(),
            None => (),
        };
        self.scheduler.enqueue_next(node_id);
    }

    fn get_node(&self, node_id: NodeId) -> &dyn Node {
        self.nodes.get(&node_id).unwrap().borrow()
    }
}

#[cfg(test)]
struct Test {
    circuit: Circuit,
    marked: HashMap<String, Rc<Cell<bool>>>,
    inputs: Vec<NodeId>,
}

#[cfg(test)]
impl Test {
    pub fn new() -> Self {
        Test {
            circuit: Circuit::new(),
            marked: HashMap::new(),
            inputs: Vec::new(),
        }
    }

    fn add_node(&mut self, node: Box<dyn Node>) {
        self.circuit.add_node(node);
    }

    fn add_input(&mut self, trigger_id: NodeId) {
        self.inputs.push(trigger_id);
    }

    fn set_trigger(&mut self, trigger_id: NodeId, active: bool) {
        self.circuit
            .nodes
            .get(&trigger_id)
            .unwrap()
            .trigger(&mut self.circuit.scheduler, active);
    }

    fn mark_wire(&mut self, name: String, wire_active: Rc<Cell<bool>>) {
        self.marked.insert(name, wire_active);
    }

    fn print_marked(&self) {
        println!("Printing marked wires:");
        for (name, wire_active) in self.marked.iter() {
            println!("{name}: {}", wire_active.get());
        }
    }

    pub fn run(&mut self, max_ticks: Ticks, debug: bool) -> RunResult {
        for ticks in 0..(max_ticks + 1) {
            if debug {
                self.print_marked()
            };
            match self.circuit.run(1) {
                RunResult::Finished { after_ticks: _ } => {
                    return RunResult::Finished { after_ticks: ticks }
                }

                RunResult::ReachedMaxTicks { max_ticks: _ } => (),
            }
        }
        if debug {
            self.print_marked()
        };
        return RunResult::ReachedMaxTicks { max_ticks };
    }

    pub fn run_all_inputs(&mut self, max_ticks: Ticks) -> RunResult {
        let num_inputs: u8 = self.inputs.len().try_into().unwrap();
        let mut input: u32 = 0;
        let max_input: u32 = 1 << num_inputs;
        let mut ticks: Tick = 0;
        // println!("Running all {num_inputs} inputs in {max_input} tests...");
        while input < max_input {
            // println!("Running input '{input}'");
            for (i, trigger_id) in self.inputs.iter().cloned().enumerate() {
                let active = (input & (1 << i)) != 0;
                self.circuit
                    .nodes
                    .get(&trigger_id)
                    .unwrap()
                    .trigger(&mut self.circuit.scheduler, active);
            }
            match self.circuit.run(max_ticks - ticks) {
                RunResult::Finished { after_ticks } => ticks += after_ticks,
                RunResult::ReachedMaxTicks { max_ticks: _ } => {
                    return RunResult::ReachedMaxTicks { max_ticks }
                }
            }
            // println!("Results:");
            // self.print_marked();
            input += 1;
        }
        RunResult::Finished { after_ticks: ticks }
    }
}

#[cfg(test)]
struct Connector {
    test: Rc<RefCell<Test>>,
    wire_id: NodeId,
    wire_active: Rc<Cell<bool>>,
}

#[cfg(test)]
impl Connector {
    fn of_wire(test: Rc<RefCell<Test>>, wire: Wire) -> Self {
        let connector = Connector {
            test: test.clone(),
            wire_id: wire.id,
            wire_active: wire.active.clone(),
        };
        test.borrow_mut().add_node(Box::new(wire));
        connector
    }

    pub fn new(test: Rc<RefCell<Test>>) -> Self {
        Self::of_wire(test, Wire::new())
    }

    fn add_output(&self, node_id: NodeId) {
        self.test
            .borrow_mut()
            .circuit
            .nodes
            .get_mut(&self.wire_id)
            .unwrap()
            .as_mut()
            .add_output(node_id)
    }

    pub fn or<'a, I>(connectors: I) -> Self
    where
        I: Iterator<Item = &'a Connector>,
    {
        let mut output_wire = Wire::new();
        let mut output_test = None;
        for connector in connectors {
            match output_test.as_ref() {
                Some(output_test) => assert!(Rc::ptr_eq(&connector.test, output_test)),
                None => output_test = Some(connector.test.clone()),
            };
            connector.add_output(output_wire.id);
            output_wire
                .inputs
                .insert(connector.wire_id, connector.wire_active.clone());
        }
        let test = output_test.unwrap();
        Self::of_wire(test, output_wire)
    }

    pub fn mark(&self, name: String) -> &Self {
        self.test
            .borrow_mut()
            .mark_wire(name, self.wire_active.clone());
        self
    }

    pub fn invert(&self) -> Self {
        let id = NodeId::new();
        let mut output_wire = Wire::new();
        let inverter = Box::new(Inverter {
            id,
            input: Some((self.wire_id, self.wire_active.clone())),
            output: Some(output_wire.id()),
            active: Rc::new(Cell::new(true)),
            next_active: Cell::new(true),
        });
        self.add_output(id);
        output_wire.inputs.insert(id, inverter.active.clone());
        self.test.borrow_mut().add_node(inverter);
        Self::of_wire(self.test.clone(), output_wire)
    }

    pub fn trigger(test: Rc<RefCell<Test>>) -> (Self, NodeId) {
        let id = NodeId::new();
        let mut wire = Wire::new();
        let trigger = Trigger {
            id,
            output: Some(wire.id),
            active: Rc::new(Cell::new(false)),
            next_active: Cell::new(false),
        };
        wire.inputs.insert(trigger.id, trigger.active.clone());
        let trigger = Box::new(trigger);
        test.borrow_mut().add_node(trigger);
        (Self::of_wire(test, wire), id)
    }

    pub fn input(test: Rc<RefCell<Test>>) -> Self {
        let (connector, trigger_id) = Self::trigger(test.clone());
        test.borrow_mut().add_input(trigger_id);
        connector
    }
}

#[cfg(test)]
mod test {
    use std::{cell::RefCell, rc::Rc};

    use crate::circuit::Test;

    use super::Connector;

    #[test]
    fn inverter_series_test() {
        let test = Rc::new(RefCell::new(Test::new()));
        Connector::new(test.clone())
            .invert()
            .mark("post-first".to_string())
            .invert()
            .mark("post-second".to_string())
            .invert()
            .mark("post-third".to_string())
            .invert()
            .mark("post-forth".to_string())
            .invert()
            .mark("post-fifth".to_string());
        let ticks = test.borrow_mut().run(100, false);
        println!("{:?}", ticks);
    }

    fn or(a: &Connector, b: &Connector) -> Connector {
        Connector::or(vec![a, b].into_iter())
    }

    fn nor(a: &Connector, b: &Connector) -> Connector {
        or(a, b).invert()
    }

    fn nand(a: &Connector, b: &Connector) -> Connector {
        or(&a.invert(), &b.invert())
    }

    fn and(a: &Connector, b: &Connector) -> Connector {
        nand(a, b).invert()
    }

    fn xor(a: &Connector, b: &Connector) -> Connector {
        nor(&and(&a, &b), &nor(&a, &b))
    }

    fn gate_test_gen(f: fn(&Connector, &Connector) -> Connector, expecteds: [bool; 4]) {
        let test = Rc::new(RefCell::new(Test::new()));
        let (a, trigger_a) = Connector::trigger(test.clone());
        let (b, trigger_b) = Connector::trigger(test.clone());
        let out = f(&a, &b);
        let expecteds = [(false, false), (false, true), (true, false), (true, true)]
            .into_iter()
            .zip(expecteds.into_iter());
        for ((in_a, in_b), expected) in expecteds {
            let mut test = test.borrow_mut();
            test.set_trigger(trigger_a.clone(), in_a);
            test.set_trigger(trigger_b.clone(), in_b);
            test.run(100, false);
            let result = out.wire_active.get();
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn gate_tests() {
        gate_test_gen(or, [false, true, true, true]);
        gate_test_gen(nor, [true, false, false, false]);
        gate_test_gen(nand, [true, true, true, false]);
        gate_test_gen(and, [false, false, false, true]);
        gate_test_gen(xor, [false, true, true, false]);
    }

    struct Adder {
        sum: Connector,
        cout: Connector,
    }

    fn adder(a: Connector, b: Connector, cin: Connector) -> Adder {
        let a_xor_b = xor(&a, &b);
        let a_and_b = and(&a, &b);
        let a_xor_b_and_cin = and(&a_xor_b, &cin);
        let sum = xor(&a_xor_b, &cin);
        let cout = or(&a_and_b, &a_xor_b_and_cin);
        Adder { sum, cout }
    }

    fn adder_chain(test: Rc<RefCell<Test>>, n: u8, cin: Connector) {
        if n > 0 {
            let a = Connector::input(test.clone());
            let b = Connector::input(test.clone());
            let Adder { sum, cout } = adder(a, b, cin);
            sum.mark(n.to_string());
            adder_chain(test, n - 1, cout)
        } else {
            cin.mark("cout".to_string());
        }
    }

    #[test]
    fn adder_test() {
        let test = Rc::new(RefCell::new(Test::new()));
        adder_chain(test.clone(), 8, Connector::new(test.clone()));
        let result = test.borrow_mut().run_all_inputs(500_000);
        println!("{result:?}");
    }

    #[test]
    fn random_adder_test() {
        let test = Rc::new(RefCell::new(Test::new()));
        adder_chain(test.clone(), 8, Connector::new(test.clone()));
        let result = test.borrow_mut().run_all_inputs(500_000);
        println!("{result:?}");
    }
}
