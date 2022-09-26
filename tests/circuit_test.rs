#[cfg(test)]
mod test {
    use std::{cell::RefCell, collections::BTreeMap, sync::Arc};

    use digisim::{
        circuit_builder::{
            ops::*, BuilderHooks, CircuitBuilder, CircuitBuilderWithHooks, Connector, NoHooks,
        },
        circuit_sim::*,
        Circuit, NodeId,
    };

    #[derive(Default, Debug)]
    struct Marks {
        marks: BTreeMap<String, NodeId>,
    }

    impl Marks {
        fn print(&self, circuit: &Circuit) {
            for (name, node_id) in self.marks.iter().by_ref() {
                println!("{}: {}", name, circuit.get_output(*node_id));
            }
        }
    }

    impl BuilderHooks for Marks {
        type MarkNodeArgs = String;

        fn mark_node(&mut self, node_id: NodeId, name: String) {
            self.marks.insert(name, node_id);
        }
    }

    #[test]
    fn inverter_series_test() {
        let builder = Arc::new(RefCell::new(CircuitBuilderWithHooks::<Marks>::default()));
        Connector::new(builder.clone())
            .invert()
            .mark("1-output".to_string())
            .invert()
            .mark("2-output".to_string())
            .invert()
            .mark("3-output".to_string())
            .invert()
            .mark("4-output".to_string())
            .invert()
            .mark("5-output".to_string());
        let mut borrow = builder.borrow_mut();
        let (circuit, marks) = borrow.build();
        let ticks = circuit.run(100);
        println!("{:?}", ticks);
        marks.print(&circuit);
    }

    fn gate_test_gen(
        name: &str,
        f: fn(Vec<&Connector<NoHooks>>) -> Connector<NoHooks>,
        expecteds: [bool; 4],
    ) {
        let builder = Arc::new(RefCell::new(CircuitBuilder::default()));
        let (a, input_a) = Connector::input(builder.clone());
        let (b, input_b) = Connector::input(builder.clone());
        let out = f(vec![&a, &b]);
        let mut borrow = builder.borrow_mut();
        let (circuit, _) = borrow.build();
        let expecteds = [(false, false), (false, true), (true, false), (true, true)]
            .into_iter()
            .zip(expecteds.into_iter());
        for ((in_a, in_b), expected) in expecteds {
            circuit.set_input(input_a, in_a);
            circuit.set_input(input_b, in_b);
            circuit.run(100);
            let result = circuit.get_output(out.output);
            assert_eq!(result, expected, "{in_a} {name} {in_b} = {expected}");
        }
    }

    #[test]
    fn gate_tests() {
        gate_test_gen("or", or, [false, true, true, true]);
        gate_test_gen("nor", nor, [true, false, false, false]);
        gate_test_gen("and", and, [false, false, false, true]);
        gate_test_gen("nand", nand, [true, true, true, false]);
        gate_test_gen("xor", xor, [false, true, true, false]);
        gate_test_gen("xnor", xnor, [true, false, false, true]);
    }
}
