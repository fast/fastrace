use fastrace::trace;

#[trace(short_name = true)]
fn sync_generic<'a, T, E>(value: &'a T) -> Result<&'a T, E>
where
    T: ?Sized,
{
    Ok(value)
}

#[trace(short_name = true)]
async fn async_generic<T>(value: T) -> impl AsRef<str>
where
    T: Into<String>,
{
    value.into()
}

#[tokio::main]
async fn main() {
    let _ = sync_generic::<_, ()>("value");
    let _ = async_generic("value").await;
}
