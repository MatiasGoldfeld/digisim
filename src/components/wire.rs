use std::ops::{BitAnd, Shl};

use num_traits::Unsigned;

use crate::{
    circuit_sim::{CircuitSim, NodeType},
    Circuit, NodeId,
};

pub struct Wire<const BITS: usize>([NodeId; BITS]);

impl<const BITS: usize> Wire<BITS> {
    pub fn new(circuit: &mut Circuit) -> Self {
        let mut contents = [NodeId::default(); BITS];
        for wire in contents.iter_mut() {
            *wire = circuit.create_node(NodeType::Or);
        }
        Wire(contents)
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
}

pub trait Signed<T> {
    fn read_signed(&self, circuit: &Circuit) -> T;
    fn set_signed(&self, circuit: &mut Circuit, val: T);
}

macro_rules! read_signed {
    ( $bits:expr, $i:ty, $u:ty ) => {
        impl Signed<$i> for Wire<$bits> {
            fn read_signed(&self, circuit: &Circuit) -> $i {
                self.read::<$u>(circuit) as $i
            }

            fn set_signed(&self, circuit: &mut Circuit, val: $i) {
                self.set(circuit, val as $u)
            }
        }
    };
}

read_signed!(8, i8, u8);
read_signed!(16, i16, u16);
read_signed!(32, i32, u32);
read_signed!(64, i64, u64);
read_signed!(128, i128, u128);
read_signed!({ usize::BITS as usize }, isize, usize);
