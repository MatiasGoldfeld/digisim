#[cfg(test)]
mod test {
    use std::{cell::RefCell, sync::Arc};

    use digisim::{
        circuit::*,
        circuit_builder::{self, ops::*},
        circuit_fast::CircuitFast as UsedCircuit,
    };

    type Test = circuit_builder::Test<UsedCircuit>;
    type Connector = circuit_builder::Connector<UsedCircuit>;

    #[test]
    fn inverter_series_test() {
        let test = Arc::new(RefCell::new(Test::new()));
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
        println!("{:?}", test.borrow().print_marked());
    }

    fn gate_test_gen(name: &str, f: fn(Vec<&Connector>) -> Connector, expecteds: [bool; 4]) {
        let test = Arc::new(RefCell::new(Test::new()));
        let (a, input_a) = Connector::trigger(test.clone());
        let (b, input_b) = Connector::trigger(test.clone());
        let out = f(vec![&a, &b]);
        let expecteds = [(false, false), (false, true), (true, false), (true, true)]
            .into_iter()
            .zip(expecteds.into_iter());
        for ((in_a, in_b), expected) in expecteds {
            {
                let mut test = test.borrow_mut();
                test.circuit.set_input(input_a, in_a);
                test.circuit.set_input(input_b, in_b);
                test.run(100, false);
            }
            let result = out.is_active();
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
