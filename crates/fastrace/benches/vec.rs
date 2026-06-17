use divan::Bencher;
use fastrace::collector::SpanRecord;

fn main() {
    divan::main();
}

#[divan::bench(args = [1, 10, 100, 1000, 10000, 100000])]
fn allocate(bencher: Bencher, len: usize) {
    bencher.bench(|| Vec::<SpanRecord>::with_capacity(len));
}

#[divan::bench(args = [1, 10, 100, 1000, 10000, 100000])]
fn deallocate(bencher: Bencher, len: usize) {
    bencher
        .with_inputs(|| Vec::<SpanRecord>::with_capacity(len))
        .bench_values(|v| {
            drop(v);
        });
}
