use std::{cell::RefCell, sync::Arc, time::Duration};

use criterion::{criterion_group, criterion_main, Criterion};
use rand::{RngCore, SeedableRng};

use digisim::{
    circuit_builder::{self, ops::*, CircuitBuilder, NoHooks},
    circuit_sim::*,
    components::adder::RippleCarryAdder,
    InputId, NodeId,
};

type Connector = circuit_builder::Connector<NoHooks>;

pub fn adder_bench<const BITS: usize>(c: &mut Criterion) {
    if BITS > 32 {
        panic!("Too large an adder!")
    };
    let name = format!("{BITS}-bit adder");
    let builder = Arc::new(RefCell::new(CircuitBuilder::default()));
    let rca = RippleCarryAdder::<BITS>::new(builder.clone(), Connector::new(builder.clone()));
    let mut borrow = builder.borrow_mut();
    let (circuit, _) = borrow.build();
    c.bench_function(&name, |b| {
        let mut rng = rand::rngs::StdRng::from_entropy();
        b.iter_batched(
            move || rng.next_u64(),
            |input| {
                for i in 0..BITS {
                    let active_a = (input & (1 << i)) != 0;
                    let active_b = (input & (1 << (i + 32))) != 0;
                    circuit.set_input(rca.input_a[i], active_a);
                    circuit.set_input(rca.input_a[i], active_b);
                }
                circuit.run_until_done();
            },
            criterion::BatchSize::SmallInput,
        )
    });
    println!("ticks: {}", circuit.tick());
}

fn adder_benches(c: &mut Criterion) {
    adder_bench::<8>(c);
    adder_bench::<16>(c);
    adder_bench::<32>(c);
}

criterion_group! {
    name = benches;
    config = Criterion::default().measurement_time(Duration::from_millis(10000));
    targets = adder_benches
}
criterion_main!(benches);
