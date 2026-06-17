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
            core::any::type_name::<T>()
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
///     let path = func_path!();
///     assert!(path.ends_with("::foo"), "{path} should end with ::foo");
/// }
/// # foo()
/// ```
#[macro_export]
macro_rules! func_path {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            core::any::type_name::<T>()
        }
        let name = type_name_of(f);
        &name[..name.len() - 3]
    }};
}

/// Get the full path of the function where the macro is invoked. Returns a `&'static str`.
#[deprecated(since = "0.7.0", note = "Please use `fastrace::func_path!()` instead")]
#[macro_export]
macro_rules! full_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            core::any::type_name::<T>()
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
///     let loc = file_location!();
///     let mut parts = loc.rsplitn(3, ':');
///     let column = parts.next().unwrap();
///     let line = parts.next().unwrap();
///     let file = parts.next().unwrap();
///     assert!(file.ends_with(".rs"), "{file} should end with .rs");
///     assert!(
///         line.parse::<u32>().is_ok(),
///         "{line} should be a valid line number"
///     );
///     assert!(
///         column.parse::<u32>().is_ok(),
///         "{column} should be a valid column number"
///     );
/// }
/// # foo()
/// ```
#[macro_export]
macro_rules! file_location {
    () => {
        core::concat!(file!(), ":", line!(), ":", column!())
    };
}
