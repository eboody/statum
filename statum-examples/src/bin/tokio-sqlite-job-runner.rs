#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    statum_examples::showcases::tokio_sqlite_job_runner::run().await
}
