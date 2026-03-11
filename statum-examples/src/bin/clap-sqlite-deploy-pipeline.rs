#[tokio::main]
async fn main() -> std::process::ExitCode {
    match statum_examples::showcases::clap_sqlite_deploy_pipeline::run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
