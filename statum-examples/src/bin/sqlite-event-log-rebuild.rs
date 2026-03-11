#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    statum_examples::showcases::sqlite_event_log_rebuild::run().await
}
