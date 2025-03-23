use anyhow::{Result, Context};
use tracing::{info, warn};
use crate::crawler::controller::CrawlerController;
use crate::cli::config::CrawlerConfig;
use std::path::PathBuf;

/// Start a new crawling job
pub async fn crawl(url: String, profile: String, depth: Option<u32>, limit: Option<u32>) -> Result<()> {
    // Load the specified profile configuration
    let mut config = CrawlerConfig::load_profile(&profile)
        .context(format!("Failed to load profile: {}", profile))?;
    
    // Override configuration with command line parameters if provided
    if let Some(d) = depth {
        config.crawler.max_depth = d;
    }
    
    if let Some(l) = limit {
        config.crawler.max_pages = l;
    }
    
    // Initialize the crawler controller
    let controller = CrawlerController::new(config).await?;
    
    // Start the crawling job
    let job_id = controller.start_job(url).await?;
    
    info!("Crawling job started with ID: {}", job_id);
    info!("Use `crawler status {}` to check the job status", job_id);
    
    Ok(())
}

/// Check the status of a crawling job
pub async fn status(job_id: String) -> Result<()> {
    // Load the controller
    let controller = CrawlerController::connect().await?;
    
    // Get the job status
    let status = controller.get_job_status(&job_id).await?;
    
    // Display status information
    println!("Job ID: {}", job_id);
    println!("Status: {}", status.state);
    println!("Pages Crawled: {}/{}", status.pages_crawled, status.pages_total);
    println!("Started: {}", status.started_at);
    println!("Last Updated: {}", status.updated_at);
    
    if !status.errors.is_empty() {
        println!("Recent Errors:");
        for error in &status.errors {
            println!("  - {}", error);
        }
    }
    
    Ok(())
}

/// Export data from a completed job
pub async fn export(job_id: String, format: String, output: Option<String>) -> Result<()> {
    // Load the controller
    let controller = CrawlerController::connect().await?;
    
    // Check if job is complete
    let status = controller.get_job_status(&job_id).await?;
    if status.state != "completed" && status.state != "failed" {
        warn!("Job is still in progress, data may be incomplete");
    }
    
    // Determine output path
    let output_path = if let Some(path) = output {
        PathBuf::from(path)
    } else {
        let extension = match format.as_str() {
            "json" => "json",
            "csv" => "csv",
            "sql" => "sql",
            _ => "data",
        };
        PathBuf::from(format!("{}.{}", job_id, extension))
    };
    
    // Export the data
    controller.export_job_data(&job_id, &format, &output_path).await?;
    
    info!("Data exported to: {}", output_path.display());
    
    Ok(())
}

/// List all available configuration profiles
pub async fn list_profiles() -> Result<()> {
    let profiles = CrawlerConfig::list_profiles().await?;
    
    println!("Available configuration profiles:");
    for profile in profiles {
        println!("  - {}", profile);
    }
    
    Ok(())
}

/// Manage a specific configuration profile
pub async fn manage_profile(profile_name: String) -> Result<()> {
    // Load the profile if it exists
    match CrawlerConfig::load_profile(&profile_name) {
        Ok(config) => {
            // Display the configuration
            println!("Profile: {}", profile_name);
            println!("{:#?}", config);
        },
        Err(_) => {
            // Profile doesn't exist, create a new one
            warn!("Profile '{}' does not exist. Creating a default profile.", profile_name);
            let config = CrawlerConfig::default();
            config.save_as_profile(&profile_name).await?;
            println!("Created default profile: {}", profile_name);
        }
    }
    
    Ok(())
}

/// Show the current configuration
pub async fn show_config() -> Result<()> {
    let config = CrawlerConfig::load_default()?;
    println!("Current configuration:");
    println!("{:#?}", config);
    
    Ok(())
}