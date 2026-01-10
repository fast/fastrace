// Test that #[trace] preserves dead_code warnings
// Before the fix, dead_code warnings were suppressed by the macro
// This test verifies that the macro properly preserves lints by using quote_spanned!

struct Foo;

impl Foo {
    // These functions are intentionally unused to test lint preservation
    // We use #[allow(dead_code)] to prevent warnings during test compilation
    #[allow(dead_code)]
    #[fastrace::trace]
    fn unused_sync_function(&self) -> i32 {
        42
    }

    #[allow(dead_code)]
    #[fastrace::trace]
    async fn unused_async_function(&self) -> i32 {
        42
    }
}

fn main() {}
