use anyhow::Result;
use tracing::{info, error};
use tracing_subscriber::{fmt, EnvFilter};

mod cli;
mod crawler;
mod browser;
mod proxy;
mod storage;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    info!("Starting Smart Crawler v{}", env!("CARGO_PKG_VERSION"));

    // Parse command line arguments
    let args = cli::parse_args();
    
    // Process commands
    match cli::process_command(args).await {
        Ok(_) => {
            info!("Command completed successfully");
            Ok(())
        }
        Err(e) => {
            error!("Command failed: {}", e);
            Err(e)
        }
    }
}