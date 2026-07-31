use fastrace::trace;

#[derive(Debug)]
struct Input {
    value: u64,
}

#[trace(short_name = true, properties = { "literal": "value", "input": "{input:?}", "escaped": "{{input}}" })]
fn f(input: &Input) -> u64 {
    input.value
}

#[test]
fn test() {
    assert_eq!(f(&Input { value: 7 }), 7);
}
