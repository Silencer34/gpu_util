//! Placeholder bench — real throughput measurements land with the
//! RadixSorter implementation.

use criterion::{criterion_group, criterion_main, Criterion};

fn placeholder(c: &mut Criterion) {
    c.bench_function("placeholder", |b| b.iter(|| 0u32));
}

criterion_group!(benches, placeholder);
criterion_main!(benches);
