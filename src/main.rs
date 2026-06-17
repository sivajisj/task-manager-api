#[tokio::main]
async fn main() {
    if let Err(e) = task_manager_api::run().await {
        eprintln!("FATAL ERROR: {:#}", e);
        std::process::exit(1);
    }
}
