use crate::{
    circuit_builder::{ops::*, BuilderHooks, Connector},
    circuit_sim::{
        CircuitSim,
        NodeType::{self, *},
    },
    Circuit, NodeId,
};

use super::wire::Wire;

// TODO:
// - Revamp Connector interface again
//   - Either make everything use it or find a better way to construct gates
//     without it
// - More efficient latches (shared input_not?)
//   - Shared input_not?
//   - Multi-dimensional cell array
// - Test out uninitialized arrays for wires and other

pub fn create_d_latch<T: BuilderHooks>(input: Connector<T>, enable: Connector<T>) -> Connector<T> {
    // TODO: Share input and input_not

    // All these [set]s are kinda hacks to initialize the latch as 0
    // to avoid the infinite loop of an undefined state.
    let q_reset = and!(nor!(input), enable);
    let q_set = and!(or!(input), enable);
    q_reset.set(false);
    q_set.set(false);

    let q = nor!(q_reset);
    let q_not = nor!(q_set);
    q.set(false);
    q.connect(&q_not);
    q_not.connect(&q);
    q
}

pub fn create_d_latch2(
    circuit: &mut Circuit,
    input_pos: NodeId,
    input_neg: NodeId,
    enable: NodeId,
    write: NodeId,
) -> NodeId {
    let q_reset = circuit.create_node(And);
    circuit.connect(input_neg, q_reset);
    circuit.connect(enable, q_reset);
    circuit.connect(write, q_reset);
    circuit.set_input(q_reset, false);

    let q_set = circuit.create_node(And);
    circuit.connect(input_pos, q_set);
    circuit.connect(enable, q_set);
    circuit.connect(write, q_set);
    circuit.set_input(q_set, false);

    let q = circuit.create_node(Nor);
    circuit.connect(q_reset, q);
    circuit.set_input(q, false);

    let q_not = circuit.create_node(Nor);
    circuit.connect(q_set, q_not);

    circuit.connect(q, q_not);
    circuit.connect(q_not, q);

    let output = circuit.create_node(And);
    circuit.connect(q, output);
    circuit.connect(enable, output);
    output
}

pub struct Sram<const ADDR_SIZE: usize, const WORD_SIZE: usize> {
    pub address: Wire<ADDR_SIZE>,
    pub input: Wire<WORD_SIZE>,
    pub output: Wire<WORD_SIZE>,
    pub write: NodeId,
}

fn create_sram_cell<const BITS: usize>(
    circuit: &mut Circuit,
    input: Wire<BITS>,
    enable: NodeId,
    write: NodeId,
) -> Wire<BITS> {
    input.map(|input| {
        let input_pos = circuit.create_node(Or);
        let input_neg = circuit.create_node(Nor);
        circuit.connect(input, input_pos);
        circuit.connect(input, input_neg);
        create_d_latch2(circuit, input_pos, input_neg, enable, write)
    })
}

impl Sram<16, 16> {
    pub fn new<const CELLS: usize>(circuit: &mut Circuit) -> Self {
        let address = Wire::new(circuit);
        let input = Wire::new(circuit);

        let write = circuit.create_node(NodeType::Or);
        // Needs to be delayed by 2 ticks to match with the address decode
        let write_delay_1 = circuit.create_node(NodeType::Or);
        let write_delay_2 = circuit.create_node(NodeType::Or);
        circuit.connect(write, write_delay_1);
        circuit.connect(write_delay_1, write_delay_2);

        let enables = address.decode::<CELLS>(circuit);
        let output = Wire::new(circuit);
        for enable in enables.iter().cloned() {
            create_sram_cell(circuit, input, enable, write_delay_2).connect(circuit, &output);
        }

        Self {
            address,
            input,
            output,
            write,
        }
    }

    pub fn new_full_2d(circuit: &mut Circuit) -> Self {
        const CELLS: usize = 1 << 16;
        let address = Wire::new(circuit);
        let input = Wire::new(circuit);

        let write = circuit.create_node(NodeType::Or);
        // Needs to be delayed by 2 ticks to match with the address decode
        let write_delay_1 = circuit.create_node(NodeType::Or);
        let write_delay_2 = circuit.create_node(NodeType::Or);
        let write_delay_3 = circuit.create_node(NodeType::Or);
        circuit.connect(write, write_delay_1);
        circuit.connect(write_delay_1, write_delay_2);
        circuit.connect(write_delay_2, write_delay_3);

        let sel0 = address.slice::<0, 8>().decode::<256>(circuit);
        let sel1 = address.slice::<8, 8>().decode::<256>(circuit);

        let output = Wire::new(circuit);
        for i in 0..CELLS {
            let i0 = i & ((1 << 8) - 1);
            let i1 = i >> 8;
            let enable = circuit.create_node(And);
            circuit.connect(sel0[i0], enable);
            circuit.connect(sel1[i1], enable);

            create_sram_cell(circuit, input, enable, write_delay_2).connect(circuit, &output);
        }

        Self {
            address,
            input,
            output,
            write,
        }
    }

    pub fn set(&self, circuit: &mut Circuit, address: u16, val: u16) {
        self.address.set(circuit, address);
        self.input.set(circuit, val);
        circuit.run_until_done();
        circuit.set_input(self.write, true);
        circuit.run_until_done();
        circuit.set_input(self.write, false);
        circuit.run(1);
    }

    pub fn get(&self, circuit: &mut Circuit, address: u16) -> u16 {
        self.address.set(circuit, address);
        circuit.run_until_done();
        self.output.read(circuit)
    }
}

#[cfg(test)]
mod test {
    use std::{cell::RefCell, sync::Arc};

    use crate::{
        circuit_builder::{CircuitBuilder, Connector},
        circuit_sim::CircuitSim,
        Circuit,
    };

    use super::{create_d_latch, Sram};

    #[test]
    fn d_latch_test() {
        let builder = Arc::new(RefCell::new(CircuitBuilder::default()));
        let (input_connector, input_id) = Connector::input(builder.clone());
        let (enable_connector, enable_id) = Connector::input(builder.clone());
        let output_id = create_d_latch(input_connector, enable_connector).output;
        let circuit = &mut builder.borrow_mut().circuit;

        let output = circuit.get_output(output_id);
        assert_eq!(output, false);
        circuit.run_until_done();
        let output = circuit.get_output(output_id);
        assert_eq!(output, false);

        circuit.set_input(enable_id, true);
        circuit.set_input(input_id, true);
        circuit.run_until_done();
        let output = circuit.get_output(output_id);
        assert_eq!(output, true);
        circuit.set_input(enable_id, false);
        circuit.run_until_done();
        let output = circuit.get_output(output_id);
        assert_eq!(output, true);

        circuit.set_input(enable_id, true);
        circuit.set_input(input_id, false);
        circuit.run_until_done();
        let output = circuit.get_output(output_id);
        assert_eq!(output, false);
        circuit.set_input(enable_id, false);
        circuit.run_until_done();
        let output = circuit.get_output(output_id);
        assert_eq!(output, false);
    }

    #[test]
    fn sram_test() {
        let mut circuit = Circuit::default();
        // let sram = Sram::new::<1024>(&mut circuit);
        let sram = Sram::new_full_2d(&mut circuit);

        assert_eq!(sram.get(&mut circuit, 12), 0);

        sram.set(&mut circuit, 12, 5);
        assert_eq!(sram.get(&mut circuit, 12), 5);

        sram.set(&mut circuit, 42, 18);
        assert_eq!(sram.get(&mut circuit, 42), 18);
        assert_eq!(sram.get(&mut circuit, 12), 5);

        sram.set(&mut circuit, 12, 99);
        assert_eq!(sram.get(&mut circuit, 12), 99);

        println!("got here!!");

        sram.set(&mut circuit, 67, 50);
        sram.set(&mut circuit, 68, 100);
        assert_eq!(sram.get(&mut circuit, 67), 50);
        assert_eq!(sram.get(&mut circuit, 68), 100);
    }
}
