#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    statum_examples::showcases::axum_sqlite_review::run().await
}
