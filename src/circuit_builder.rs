use std::{cell::RefCell, collections::HashMap, sync::Arc};

use crate::circuit::*;

pub struct Test<C: Circuit> {
    pub circuit: C,
    marked: HashMap<String, C::NodeId>,
    pub inputs: Vec<C::InputId>,
}

impl<C: Circuit> Test<C> {
    pub fn new() -> Self {
        Test {
            circuit: Circuit::new(),
            marked: HashMap::new(),
            inputs: Vec::new(),
        }
    }

    fn add_input(&mut self, input_id: C::InputId) {
        self.inputs.push(input_id);
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
                self.circuit.set_input(trigger_id, active);
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
        let output = test.borrow_mut().circuit.or();
        Self::from_output(test, output)
    }

    fn gate_gen<'a>(f: fn(&mut C) -> C::NodeId, inputs: Vec<&'a Self>) -> Self {
        let test = inputs[0].test.clone();
        let circuit = &mut test.borrow_mut().circuit;
        let output = f(circuit);
        for input in inputs {
            assert!(Arc::ptr_eq(&test, &input.test));
            circuit.connect(input.output, output);
        }
        Self::from_output(test.clone(), output)
    }

    pub fn mark(&self, name: String) -> &Self {
        self.test.borrow_mut().mark_wire(name, self.output);
        self
    }

    pub fn invert(&self) -> Self {
        let circuit = &mut self.test.borrow_mut().circuit;
        let inverter = circuit.nor();
        circuit.connect(self.output, inverter);
        Self::from_output(self.test.clone(), inverter)
    }

    pub fn trigger(test: Arc<RefCell<Test<C>>>) -> (Self, C::InputId) {
        let circuit = &mut test.borrow_mut().circuit;
        let trigger = circuit.input();
        (Self::from_output(test.clone(), trigger.into()), trigger)
    }

    pub fn input(test: Arc<RefCell<Test<C>>>) -> Self {
        let (connector, input_id) = Self::trigger(test.clone());
        test.borrow_mut().add_input(input_id);
        connector
    }

    pub fn is_active(&self) -> bool {
        self.test.borrow().circuit.is_active(self.output)
    }
}

pub mod ops {
    use crate::circuit::Circuit;

    use super::Connector;

    pub use crate::{and, nand, nor, or, xor, xnor};

    pub fn or<C: Circuit>(inputs: Vec<&Connector<C>>) -> Connector<C> {
        Connector::gate_gen(C::or, inputs)
    }

    pub fn nor<C: Circuit>(inputs: Vec<&Connector<C>>) -> Connector<C> {
        Connector::gate_gen(C::nor, inputs)
    }

    pub fn and<C: Circuit>(inputs: Vec<&Connector<C>>) -> Connector<C> {
        Connector::gate_gen(C::and, inputs)
    }

    pub fn nand<C: Circuit>(inputs: Vec<&Connector<C>>) -> Connector<C> {
        Connector::gate_gen(C::nand, inputs)
    }

    pub fn xor<C: Circuit>(inputs: Vec<&Connector<C>>) -> Connector<C> {
        Connector::gate_gen(C::xor, inputs)
    }

    pub fn xnor<C: Circuit>(inputs: Vec<&Connector<C>>) -> Connector<C> {
        Connector::gate_gen(C::xnor, inputs)
    }

    #[macro_export]
    macro_rules! or {
        ( $( $inputs:expr ),+ ) => {
            or(vec!($(&$inputs),+))
        };
    }

    #[macro_export]
    macro_rules! nor {
        ( $( $inputs:expr ),+ ) => {
            nor(vec!($(&$inputs),+))
        };
    }

    #[macro_export]
    macro_rules! and {
        ( $( $inputs:expr ),+ ) => {
            and(vec!($(&$inputs),+))
        };
    }

    #[macro_export]
    macro_rules! nand {
        ( $( $inputs:expr ),+ ) => {
            nand(vec!($(&$inputs),+))
        };
    }

    #[macro_export]
    macro_rules! xor {
        ( $( $inputs:expr ),+ ) => {
            xor(vec!($(&$inputs),+))
        };
    }

    #[macro_export]
    macro_rules! xnor {
        ( $( $inputs:expr ),+ ) => {
            xnor(vec!($(&$inputs),+))
        };
    }
}
