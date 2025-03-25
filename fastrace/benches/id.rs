use divan::Bencher;
use fastrace::collector::SpanId;

fn main() {
    divan::main();
}

#[divan::bench(args = [1, 10, 100, 1000, 10000])]
fn random(bencher: Bencher, len: usize) {
    bencher.bench(|| {
        for _ in 0..len {
            divan::black_box(SpanId::random());
        }
    })
}

#[divan::bench(args = [1, 10, 100, 1000, 10000])]
fn thread_local(bencher: Bencher, len: usize) {
    bencher.bench(|| {
        for _ in 0..len {
            divan::black_box(SpanId::next_id());
        }
    })
}
