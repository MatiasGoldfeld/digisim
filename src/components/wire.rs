use std::ops::{BitAnd, Index, Shl};

use num_traits::Unsigned;

use crate::{
    circuit_sim::{CircuitSim, NodeType},
    Circuit, NodeId,
};

#[derive(Clone, Copy)]
pub struct Wire<const BITS: usize>([NodeId; BITS]);

impl<const BITS: usize> Wire<BITS> {
    pub fn uninit() -> Self {
        // TODO: See if going on nightly and using MaybeUninit is better?
        Wire([NodeId::default(); BITS])
    }

    pub fn of_node_ids<F: FnMut(usize) -> NodeId>(mut f: F) -> Self {
        let mut wire = Self::uninit();
        for (bit, node_id) in wire.0.iter_mut().enumerate() {
            *node_id = f(bit);
        }
        wire
    }

    pub fn new(circuit: &mut Circuit) -> Self {
        Self::of_node_ids(|_| circuit.create_node(NodeType::Or))
    }

    pub fn read<T>(&self, circuit: &Circuit) -> T
    where
        T: Unsigned + Shl<usize, Output = T>,
    {
        let mut sum = T::zero();
        for (bit, node_id) in self.0.iter().cloned().enumerate() {
            if circuit.get_output(node_id) {
                sum = sum + (T::one() << bit);
            }
        }
        sum
    }

    pub fn set<T>(&self, circuit: &mut Circuit, val: T)
    where
        T: Unsigned + Copy + BitAnd<T, Output = T> + Shl<usize, Output = T>,
    {
        for (bit, node_id) in self.0.iter().cloned().enumerate() {
            let bit_val = (val & (T::one() << bit)).is_one();
            circuit.set_input(node_id, bit_val);
        }
    }

    pub fn connect(&self, circuit: &mut Circuit, output: &Self) {
        for (input, output) in self.iter().cloned().zip(output.iter().cloned()) {
            circuit.connect(input, output);
        }
    }

    pub fn slice<const START: usize, const LEN: usize>(&self) -> Wire<LEN> {
        assert!(START + LEN <= BITS);
        let mut wire = Wire::uninit();
        wire.0.copy_from_slice(&self.0[START..START + LEN]);
        wire
    }

    pub fn iter(&self) -> std::slice::Iter<NodeId> {
        self.0.iter()
    }

    pub fn map<F: FnMut(NodeId) -> NodeId>(&self, mut f: F) -> Wire<BITS> {
        let mut wire = *self;
        for node_id in wire.0.iter_mut() {
            *node_id = f(*node_id);
        }
        wire
    }

    fn map_gate(&self, circuit: &mut Circuit, node_type: NodeType) -> Wire<BITS> {
        self.map(|input| {
            let output = circuit.create_node(node_type);
            circuit.connect(input, output);
            output
        })
    }

    pub fn buffer(&self, circuit: &mut Circuit) -> Wire<BITS> {
        self.map_gate(circuit, NodeType::Or)
    }

    pub fn invert(&self, circuit: &mut Circuit) -> Wire<BITS> {
        self.map_gate(circuit, NodeType::Nor)
    }

    pub fn enable(&self, circuit: &mut Circuit, enable: NodeId) -> Wire<BITS> {
        let wire = self.map_gate(circuit, NodeType::And);
        for output in wire.0.iter().cloned() {
            circuit.connect(enable, output);
        }
        wire
    }

    pub fn decode<const OUTPUTS: usize>(&self, circuit: &mut Circuit) -> Wire<OUTPUTS> {
        assert!(OUTPUTS <= (1 << BITS));
        let wire_pos = self.buffer(circuit);
        let wire_neg = self.invert(circuit);

        let mut wire = Wire::uninit();
        for (i, output) in wire.0.iter_mut().enumerate() {
            *output = circuit.create_node(NodeType::And);
            for bit in 0..BITS {
                if i & (1 << bit) == 0 {
                    circuit.connect(wire_neg.0[bit], *output);
                } else {
                    circuit.connect(wire_pos.0[bit], *output);
                }
            }
        }
        wire
    }
}

impl<const BITS: usize> Index<usize> for Wire<BITS> {
    type Output = NodeId;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

pub trait Signed<T> {
    fn read_signed(&self, circuit: &Circuit) -> T;
    fn set_signed(&self, circuit: &mut Circuit, val: T);
}

macro_rules! read_signed {
    ( $i:ty, $u:ty ) => {
        impl Signed<$i> for Wire<{ <$u>::BITS as usize }> {
            fn read_signed(&self, circuit: &Circuit) -> $i {
                self.read::<$u>(circuit) as $i
            }

            fn set_signed(&self, circuit: &mut Circuit, val: $i) {
                self.set(circuit, val as $u)
            }
        }
    };
}

read_signed!(i8, u8);
read_signed!(i16, u16);
read_signed!(i32, u32);
read_signed!(i64, u64);
read_signed!(i128, u128);
read_signed!(isize, usize);
