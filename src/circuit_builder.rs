use std::{cell::RefCell, collections::HashMap, sync::Arc};

use crate::circuit::*;

pub struct Test<C: Circuit> {
    pub circuit: C,
    marked: HashMap<String, C::NodeId>,
    pub inputs: Vec<C::NodeId>,
}

impl<C: Circuit> Test<C> {
    pub fn new() -> Self {
        Test {
            circuit: Circuit::new(),
            marked: HashMap::new(),
            inputs: Vec::new(),
        }
    }

    fn add_input(&mut self, trigger_id: C::NodeId) {
        self.inputs.push(trigger_id);
    }

    fn mark_wire(&mut self, name: String, node_id: C::NodeId) {
        self.marked.insert(name, node_id);
    }

    pub fn print_marked(&self) {
        println!("Printing marked wires:");
        for (name, node_id) in self.marked.iter() {
            println!("{name}: {}", self.circuit.is_active(*node_id));
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
                self.circuit.trigger_node(trigger_id, active);
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

pub struct Connector<C: Circuit> {
    test: Arc<RefCell<Test<C>>>,
    output: C::NodeId,
}

impl<C: Circuit> Connector<C> {
    fn from_output(test: Arc<RefCell<Test<C>>>, output: C::NodeId) -> Self {
        Connector { test, output }
    }

    pub fn new(test: Arc<RefCell<Test<C>>>) -> Self {
        let output = test.borrow_mut().circuit.wire();
        Self::from_output(test, output)
    }

    fn or<'a>(connectors: Vec<&'a Self>) -> Self {
        let test = connectors[0].test.clone();
        let circuit = &mut test.borrow_mut().circuit;
        let wire = circuit.wire();
        for connector in connectors {
            assert!(Arc::ptr_eq(&test, &connector.test));
            circuit.connect(connector.output, wire);
        }
        Self::from_output(test.clone(), wire)
    }

    pub fn mark(&self, name: String) -> &Self {
        self.test.borrow_mut().mark_wire(name, self.output);
        self
    }

    pub fn invert(&self) -> Self {
        let circuit = &mut self.test.borrow_mut().circuit;
        let inverter = circuit.inverter();
        let wire = circuit.wire();
        circuit.connect(self.output, inverter);
        circuit.connect(inverter, wire);
        Self::from_output(self.test.clone(), wire)
    }

    pub fn trigger(test: Arc<RefCell<Test<C>>>) -> (Self, C::NodeId) {
        let circuit = &mut test.borrow_mut().circuit;
        let trigger = circuit.trigger();
        let wire = circuit.wire();
        circuit.connect(trigger, wire);
        (Self::from_output(test.clone(), wire), trigger)
    }

    pub fn input(test: Arc<RefCell<Test<C>>>) -> Self {
        let (connector, trigger) = Self::trigger(test.clone());
        test.borrow_mut().add_input(trigger);
        connector
    }

    pub fn is_active(&self) -> bool {
        self.test.borrow().circuit.is_active(self.output)
    }
}

pub mod ops {
    use crate::circuit::Circuit;

    use super::Connector;

    pub fn or<C: Circuit>(a: &Connector<C>, b: &Connector<C>) -> Connector<C> {
        Connector::or(vec![a, b])
    }

    pub fn nor<C: Circuit>(a: &Connector<C>, b: &Connector<C>) -> Connector<C> {
        or(a, b).invert()
    }

    pub fn nand<C: Circuit>(a: &Connector<C>, b: &Connector<C>) -> Connector<C> {
        or(&a.invert(), &b.invert())
    }

    pub fn and<C: Circuit>(a: &Connector<C>, b: &Connector<C>) -> Connector<C> {
        nand(a, b).invert()
    }

    pub fn xor<C: Circuit>(a: &Connector<C>, b: &Connector<C>) -> Connector<C> {
        nor(&and(&a, &b), &nor(&a, &b))
    }
}
