use fastrace::trace;

struct Worker(String);

impl Worker {
    #[trace(short_name = true)]
    fn sync_method(&self) -> &str {
        &self.0
    }

    #[trace(short_name = true)]
    async fn async_method(&mut self, suffix: &str) -> String {
        self.0.push_str(suffix);
        self.0.clone()
    }
}

#[tokio::test]
async fn test() {
    let mut worker = Worker(String::from("fast"));
    assert_eq!(worker.sync_method(), "fast");
    assert_eq!(worker.async_method("race").await, "fastrace");
}
