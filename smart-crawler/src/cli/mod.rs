pub mod commands;
pub mod config;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a new crawling job
    Crawl {
        /// Target URL to start crawling from
        #[arg(required = true)]
        url: String,
        
        /// Site profile to use
        #[arg(short, long, default_value = "general")]
        profile: String,
        
        /// Maximum crawling depth
        #[arg(short, long)]
        depth: Option<u32>,
        
        /// Maximum number of pages to crawl
        #[arg(short, long)]
        limit: Option<u32>,
    },
    
    /// Check status of a crawling job
    Status {
        /// Job ID to check status for
        #[arg(required = true)]
        job_id: String,
    },
    
    /// Export data from a completed job
    Export {
        /// Job ID to export data from
        #[arg(required = true)]
        job_id: String,
        
        /// Export format (csv, json, sql)
        #[arg(short, long, default_value = "json")]
        format: String,
        
        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
    },
    
    /// Manage configuration profiles
    Config {
        /// Profile name to manage
        #[arg(required = false)]
        profile: Option<String>,
        
        /// List all available profiles
        #[arg(short, long)]
        list: bool,
    },
}

/// Parse command line arguments
pub fn parse_args() -> Cli {
    Cli::parse()
}

/// Process the command
pub async fn process_command(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Crawl { url, profile, depth, limit } => {
            info!("Starting crawl on {} with profile {}", url, profile);
            commands::crawl(url, profile, depth, limit).await
        },
        Commands::Status { job_id } => {
            info!("Checking status for job {}", job_id);
            commands::status(job_id).await
        },
        Commands::Export { job_id, format, output } => {
            info!("Exporting job {} as {}", job_id, format);
            commands::export(job_id, format, output).await
        },
        Commands::Config { profile, list } => {
            if list {
                info!("Listing all configuration profiles");
                commands::list_profiles().await
            } else if let Some(profile_name) = profile {
                info!("Managing configuration profile: {}", profile_name);
                commands::manage_profile(profile_name).await
            } else {
                info!("Showing current configuration");
                commands::show_config().await
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }
}