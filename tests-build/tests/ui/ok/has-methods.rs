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

#[tokio::main]
async fn main() {
    let mut worker = Worker(String::from("fast"));
    let _ = worker.sync_method();
    let _ = worker.async_method("race").await;
}
