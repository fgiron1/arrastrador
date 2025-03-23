use anyhow::{Result, Context};
use std::path::PathBuf;
use tracing::{info, warn};

use crate::browser::remote::RemoteBrowserService;

/// Upload a custom script for a domain
pub async fn upload_script(domain: String, script_path: PathBuf) -> Result<()> {
    // Create a script manager
    let browser_service = RemoteBrowserService::new();
    let script_manager = browser_service.script_manager();
    
    // Validate the script file
    if !script_path.exists() {
        anyhow::bail!("Script file not found: {}", script_path.display());
    }
    
    if script_path.extension().map_or(false, |ext| ext != "py") {
        warn!("Script file doesn't have a .py extension: {}", script_path.display());
    }
    
    // Upload the script
    script_manager.upload_script(&domain, &script_path).await?;
    
    info!("Script uploaded successfully for domain: {}", domain);
    
    Ok(())
}

/// List all available custom scripts
pub async fn list_scripts() -> Result<()> {
    // Create a script manager
    let browser_service = RemoteBrowserService::new();
    let script_manager = browser_service.script_manager();
    
    // Get the list of scripts
    let scripts = script_manager.list_scripts().await?;
    
    println!("Available custom scripts:");
    if scripts.is_empty() {
        println!("  No custom scripts found.");
    } else {
        for script in scripts {
            println!("  - {}", script);
        }
    }
    
    Ok(())
}