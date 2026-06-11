use fastrace::trace;

#[derive(Debug)]
struct Input {
    value: u64,
}

#[trace(short_name = true, properties = { "literal": "value", "input": "{input:?}", "escaped": "{{input}}" })]
fn f(input: &Input) -> u64 {
    input.value
}

fn main() {
    let _ = f(&Input { value: 7 });
}
