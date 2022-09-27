use crate::Circuit;

use super::wire::Wire;

pub fn create_n_to_1_mux<const BITS: usize, const N: usize, const SELECT_BITS: usize>(
    circuit: &mut Circuit,
    inputs: [Wire<BITS>; N],
    select: Wire<SELECT_BITS>,
) -> Wire<BITS> {
    assert!(N <= (1 << SELECT_BITS));
    let output = Wire::new(circuit);
    let decoded = select.decode::<N>(circuit);
    for (i, enable) in decoded.iter().cloned().enumerate() {
        inputs[i].enable(circuit, enable).connect(circuit, &output);
    }
    output
}
