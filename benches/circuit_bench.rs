use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::format,
    ops::DerefMut,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use criterion::{black_box, criterion_group, criterion_main, Bencher, BenchmarkId, Criterion};
use rand::{RngCore, SeedableRng};

use transist::{
    circuit::*,
    circuit_builder::{self, ops::*},
    // circuit_fast::CircuitFast as UsedCircuit,
    circuit_fast::CircuitFast as UsedCircuit,
};

type Test = circuit_builder::Test<UsedCircuit>;
type Connector = circuit_builder::Connector<UsedCircuit>;

struct Adder {
    sum: Connector,
    cout: Connector,
}

fn adder(a: Connector, b: Connector, cin: Connector) -> Adder {
    let a_xor_b = xor(&a, &b);
    let a_and_b = and(&a, &b);
    let a_xor_b_and_cin = and(&a_xor_b, &cin);
    let sum = xor(&a_xor_b, &cin);
    let cout = or(&a_and_b, &a_xor_b_and_cin);
    Adder { sum, cout }
}

fn adder_chain(test: Arc<RefCell<Test>>, n: u8, cin: Connector) {
    if n > 0 {
        let a = Connector::input(test.clone());
        let b = Connector::input(test.clone());
        let Adder { sum, cout } = adder(a, b, cin);
        sum.mark(n.to_string());
        adder_chain(test, n - 1, cout)
    } else {
        cin.mark("cout".to_string());
    }
}

pub fn bench_rand_inputs(b: &mut Bencher, test: Arc<RefCell<Test>>) {
    let num_inputs: u8 = test.borrow().inputs.len().try_into().unwrap();
    if num_inputs > 64 {
        panic!("Too many inputs!")
    };
    let mut rng = rand::rngs::StdRng::from_entropy();
    b.iter_batched(
        || {
            let input = rng.next_u64();
            let mut borrow = test.borrow_mut();
            let test = borrow.deref_mut();
            for (i, trigger_id) in test.inputs.iter().cloned().enumerate() {
                let active = (input & (1 << i)) != 0;
                test.circuit.trigger_node(trigger_id, active);
            }
        },
        |()| {
            test.borrow_mut().circuit.run_until_done();
        },
        criterion::BatchSize::PerIteration,
    )
}

fn adder_bench(c: &mut Criterion, bit_count: u8) {
    let name = format!("{bit_count}-bit adder");
    let test = Arc::new(RefCell::new(Test::new()));
    adder_chain(test.clone(), bit_count, Connector::new(test.clone()));
    c.bench_function(&name, |b| bench_rand_inputs(b, test.clone()));
}

fn adder_benches(c: &mut Criterion) {
    adder_bench(c, 8);
    adder_bench(c, 16);
}

criterion_group! {
    name = benches;
    config = Criterion::default().measurement_time(Duration::from_millis(10000));
    targets = adder_benches
}
criterion_main!(benches);
