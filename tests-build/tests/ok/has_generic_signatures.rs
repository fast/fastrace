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

#[tokio::test]
async fn test() {
    assert_eq!(sync_generic::<_, ()>("value"), Ok("value"));
    assert_eq!(async_generic("value").await.as_ref(), "value");
}
