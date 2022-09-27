use std::{cell::RefCell, sync::Arc};

use crate::circuit_sim::*;
use crate::{Circuit, InputId, NodeId};

pub trait BuilderHooks: Default {
    fn create_node_hook(&mut self, _node_id: NodeId) {}
    fn create_input_hook(&mut self, _input_id: InputId) {}
    fn connect_hook(&mut self, _input: NodeId, _output: NodeId) {}

    type MarkNodeArgs;
    fn mark_node(&mut self, _node_id: NodeId, _args: Self::MarkNodeArgs) {}
}

#[derive(Default)]
pub struct NoHooks;
impl BuilderHooks for NoHooks {
    type MarkNodeArgs = ();
}

pub type CircuitBuilder = CircuitBuilderWithHooks<NoHooks>;

#[derive(Default)]
pub struct CircuitBuilderWithHooks<T: BuilderHooks> {
    pub circuit: Circuit,
    hooks: T,
}

impl<T: BuilderHooks> CircuitBuilderWithHooks<T> {
    fn create_node(&mut self, node_type: NodeType) -> NodeId {
        let node_id = self.circuit.create_node(node_type);
        self.hooks.create_node_hook(node_id);
        node_id
    }

    fn create_input(&mut self) -> InputId {
        let input_id = self.circuit.create_input();
        self.hooks.create_node_hook(input_id);
        self.hooks.create_input_hook(input_id);
        input_id
    }

    fn connect(&mut self, input: NodeId, output: NodeId) {
        self.circuit.connect(input, output);
        self.hooks.connect_hook(input, output);
    }

    fn mark_node(&mut self, node_id: NodeId, args: T::MarkNodeArgs) {
        self.hooks.mark_node(node_id, args);
    }

    pub fn build(&mut self) -> (&mut Circuit, &mut T) {
        (&mut self.circuit, &mut self.hooks)
    }
}

pub struct Connector<T: BuilderHooks> {
    builder: Arc<RefCell<CircuitBuilderWithHooks<T>>>,
    pub output: NodeId,
}

impl<T: BuilderHooks> Connector<T> {
    fn from_output(builder: Arc<RefCell<CircuitBuilderWithHooks<T>>>, output: NodeId) -> Self {
        Connector { builder, output }
    }

    pub fn new(builder: Arc<RefCell<CircuitBuilderWithHooks<T>>>) -> Self {
        let output = builder.borrow_mut().create_node(NodeType::Or);
        Self::from_output(builder.clone(), output)
    }

    pub fn input(builder: Arc<RefCell<CircuitBuilderWithHooks<T>>>) -> (Self, InputId) {
        let mut builder_mut = builder.borrow_mut();
        let input_id = builder_mut.create_input();
        (Self::from_output(builder.clone(), input_id), input_id)
    }

    pub fn input_ignore(builder: Arc<RefCell<CircuitBuilderWithHooks<T>>>) -> Self {
        let (connector, _input_id) = Self::input(builder);
        connector
    }

    fn gate_gen<'a>(node_type: NodeType, inputs: &[&'a Self]) -> Self {
        let builder = inputs[0].builder.clone();
        let mut builder_mut = builder.borrow_mut();
        let output = builder_mut.create_node(node_type);
        for input in inputs {
            assert!(Arc::ptr_eq(&builder, &input.builder));
            let input = input.output;
            builder_mut.connect(input, output);
        }
        Self::from_output(builder.clone(), output)
    }

    pub fn mark(&self, args: T::MarkNodeArgs) -> &Self {
        self.builder.borrow_mut().mark_node(self.output, args);
        self
    }

    pub fn invert(&self) -> Self {
        let mut builder_mut = self.builder.borrow_mut();
        let inverter = builder_mut.create_node(NodeType::Nor);
        builder_mut.connect(self.output, inverter);
        Self::from_output(self.builder.clone(), inverter)
    }

    pub fn connect(&self, output: &Connector<T>) {
        self.builder
            .borrow_mut()
            .connect(self.output, output.output);
    }

    pub fn set(&self, val: bool) {
        self.builder
            .borrow_mut()
            .circuit
            .set_input(self.output, val);
    }

    pub fn get_output(&self) -> bool {
        self.builder.borrow().circuit.get_output(self.output)
    }
}

pub mod ops {
    use crate::circuit_sim::NodeType;

    use super::{BuilderHooks, Connector};

    pub use crate::{and, nand, nor, or, xnor, xor};

    macro_rules! gate_fn_gen {
        ( $gate_lowercase:ident, $gate_uppercase:ident ) => {
            pub fn $gate_lowercase<T: BuilderHooks>(inputs: Vec<&Connector<T>>) -> Connector<T> {
                Connector::gate_gen(NodeType::$gate_uppercase, &inputs)
            }
        };
    }

    gate_fn_gen!(or, Or);
    gate_fn_gen!(nor, Nor);
    gate_fn_gen!(and, And);
    gate_fn_gen!(nand, Nand);
    gate_fn_gen!(xor, Xor);
    gate_fn_gen!(xnor, Xnor);

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
