//! Benchmarks for unhwp parsing performance.

use criterion::{criterion_group, criterion_main, Criterion};

fn benchmark_placeholder(_c: &mut Criterion) {
    // Placeholder benchmark - will be implemented when sample files are available
}

criterion_group!(benches, benchmark_placeholder);
criterion_main!(benches);
