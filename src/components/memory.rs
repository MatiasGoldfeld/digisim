use crate::circuit_builder::{ops::*, BuilderHooks, Connector};

pub fn create_d_latch<T: BuilderHooks>(input: Connector<T>, enable: Connector<T>) -> Connector<T> {
    let q = nor!(and!(nor!(input), enable));
    let q_not = nor!(and!(input, enable));
    q.connect(&q_not);
    q_not.connect(&q);
    q
}

#[cfg(test)]
mod test {
    use std::{cell::RefCell, sync::Arc};

    use crate::{
        circuit_builder::{CircuitBuilder, Connector},
        circuit_sim::CircuitSim,
    };

    use super::create_d_latch;

    #[test]
    fn d_latch_test() {
        let builder = Arc::new(RefCell::new(CircuitBuilder::default()));
        let (input_connector, input_id) = Connector::input(builder.clone());
        let (enable_connector, enable_id) = Connector::input(builder.clone());
        let output_id = create_d_latch(input_connector, enable_connector).output;
        let circuit = &mut builder.borrow_mut().circuit;

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
}
