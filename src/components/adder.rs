use std::{cell::RefCell, sync::Arc};

use crate::{
    circuit_builder::{self, ops::*, CircuitBuilder, NoHooks},
    InputId, NodeId,
};

type Connector = circuit_builder::Connector<NoHooks>;

struct Adder {
    sum: Connector,
    cout: Connector,
}

fn adder(a: Connector, b: Connector, cin: Connector) -> Adder {
    let sum = xor!(a, b, cin);
    let cout = or!(and!(a, b), and!(a, cin), and!(b, cin));
    Adder { sum, cout }
}

pub struct RippleCarryAdder<const BITS: usize> {
    pub input_a: [InputId; BITS],
    pub input_b: [InputId; BITS],
    pub cin: InputId,
    pub cout: NodeId,
    pub sum: [NodeId; BITS],
}

impl<const BITS: usize> RippleCarryAdder<BITS> {
    pub fn new(builder: Arc<RefCell<CircuitBuilder>>, cin: Connector) -> RippleCarryAdder<BITS> {
        assert!(BITS > 0);

        let mut rca = Self {
            input_a: [InputId::default(); BITS],
            input_b: [InputId::default(); BITS],
            cin: Default::default(),
            cout: Default::default(),
            sum: [InputId::default(); BITS],
        };

        rca.cin = cin.output;
        let mut carry = cin;
        for i in 0..BITS {
            let a = Connector::input_ignore(builder.clone());
            let b = Connector::input_ignore(builder.clone());
            rca.input_a[i] = a.output;
            rca.input_b[i] = b.output;
            let Adder { sum, cout } = adder(a, b, carry);
            rca.sum[i] = sum.output;
            carry = cout;
        }
        rca.cout = carry.output;
        rca
    }
}

#[cfg(test)]
mod test {
    use rand::RngCore;

    use crate::{
        circuit_builder::{CircuitBuilder, Connector},
        circuit_sim::CircuitSim,
        Circuit,
    };
    use std::{cell::RefCell, sync::Arc};

    use super::{adder, RippleCarryAdder};

    fn test_adder(a: bool, b: bool, cin: bool) {
        let builder = Arc::new(RefCell::new(CircuitBuilder::default()));
        let (ca, ia) = Connector::input(builder.clone());
        let (cb, ib) = Connector::input(builder.clone());
        let (ccin, icin) = Connector::input(builder.clone());
        let adder = adder(ca, cb, ccin);
        {
            let mut borrow = builder.borrow_mut();
            let (circuit, _) = borrow.build();
            circuit.set_input(ia, a);
            circuit.set_input(ib, b);
            circuit.set_input(icin, cin);
            circuit.run_until_done();
        }
        assert_eq!(adder.sum.get_output(), a ^ b ^ cin);
        assert_eq!(
            adder.cout.get_output(),
            (a && b) || (a && cin) || (b && cin)
        )
    }

    #[test]
    fn adder_tests() {
        test_adder(false, false, false);
        test_adder(true, false, false);
        test_adder(false, true, false);
        test_adder(true, true, false);
        test_adder(false, false, true);
        test_adder(true, false, true);
        test_adder(false, true, true);
        test_adder(true, true, true);
    }

    fn test_rca_add<const BITS: usize>(
        circuit: &mut Circuit,
        rca: &RippleCarryAdder<BITS>,
        a: u64,
        b: u64,
    ) {
        let overflow = 1 << BITS;
        assert!(a < overflow && b < overflow);

        for i in 0..BITS {
            let active_a = (a & (1 << i)) != 0;
            let active_b = (b & (1 << i)) != 0;
            circuit.set_input(rca.input_a[i], active_a);
            circuit.set_input(rca.input_b[i], active_b);
        }
        circuit.run_until_done();

        let expected_sum = a + b;
        let (expected_sum, expected_cout) = if expected_sum < overflow {
            (expected_sum, false)
        } else {
            (expected_sum - overflow, true)
        };

        let mut sum = 0;
        for i in 0..BITS {
            if circuit.get_output(rca.sum[i]) {
                sum += 1 << i;
            }
        }
        let cout = circuit.get_output(rca.cout);

        assert_eq!(sum, expected_sum, "{a} + {b} = {expected_sum}");
        assert_eq!(
            cout, expected_cout,
            "{a} + {b} with {BITS} bits has cout: {expected_cout}"
        );
    }

    #[test]
    fn rca_tests() {
        let builder = Arc::new(RefCell::new(CircuitBuilder::default()));
        let rca = RippleCarryAdder::<16>::new(builder.clone(), Connector::new(builder.clone()));
        let mut borrow = builder.borrow_mut();
        let (circuit, _) = borrow.build();
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let a = rng.next_u32() as u16;
            let b = rng.next_u32() as u16;
            test_rca_add(circuit, &rca, a as u64, b as u64);
        }
    }
}
