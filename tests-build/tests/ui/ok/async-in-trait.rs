#![deny(warnings)]
#![allow(clippy::all)]

trait MyTrait {
    async fn work(&self) -> usize;
}

struct MyStruct;

impl MyTrait for MyStruct {
    // #[logcall::logcall("info")]
    #[fastrace::trace]
    async fn work(&self) -> usize {
        unimplemented!()
    }
}

#[test]
fn test() {
    fn assert_method<T: MyTrait>(value: &T) {
        let _ = T::work(value);
    }

    assert_method(&MyStruct);
}
