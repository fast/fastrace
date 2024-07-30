// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

/// Get the name of the function where the macro is invoked. Returns a `&'static str`.
///
/// # Example
///
/// ```
/// use fastrace::func_name;
///
/// fn foo() {
///     assert_eq!(func_name!(), "foo");
/// }
/// # foo()
/// ```
#[macro_export]
macro_rules! func_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        let name = &name[..name.len() - 3];
        name.rsplit("::")
            .find(|name| *name != "{{closure}}")
            .unwrap()
    }};
}

/// Get the full path of the function where the macro is invoked. Returns a `&'static str`.
///
/// # Example
///
/// ```
/// use fastrace::func_path;
///
/// fn foo() {
///    assert_eq!(func_path!(), "rust_out::main::_doctest_main_fastrace_src_macros_rs_34_0::foo");
/// }
/// # foo()
#[macro_export]
macro_rules! func_path {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        let name = &name[..name.len() - 3];
        name.trim_end_matches("::{{closure}}")
    }};
}

/// Get the full path of the function where the macro is invoked. Returns a `&'static str`.
#[deprecated(since = "0.7.0", note = "Please use `fastrace::func_path!()` instead")]
#[macro_export]
macro_rules! full_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        let name = &name[..name.len() - 3];
        name.trim_end_matches("::{{closure}}")
    }};
}

/// Get the source file location where the macro is invoked. Returns a `&'static str`.
///
/// # Example
///
/// ```
/// use fastrace::file_location;
///
/// fn foo() {
///    assert_eq!(file_location!(), "fastrace/src/macros.rs:8:15");
/// }
/// # #[cfg(not(target_os = "windows"))]
/// # foo()
#[macro_export]
macro_rules! file_location {
    () => {
        std::concat!(file!(), ":", line!(), ":", column!())
    };
}
