use anyhow::{Result, Context};
use reqwest::Client;
use std::path::Path;
use std::fs;
use tracing::{info, error};

/// Script manager for browser service
pub struct ScriptManager {
    client: Client,
    base_url: String,
}

impl ScriptManager {
    /// Create a new script manager
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        }
    }
    
    /// Upload a custom script for a specific domain
    pub async fn upload_script(&self, domain: &str, script_path: &Path) -> Result<()> {
        let script_content = fs::read_to_string(script_path)
            .context(format!("Failed to read script file: {}", script_path.display()))?;
        
        let endpoint = format!("{}/script/{}", self.base_url, domain);
        
        let response = self.client.put(&endpoint)
            .json(&serde_json::json!({
                "script": script_content
            }))
            .send()
            .await
            .context("Failed to send script to browser service")?;
            
        if response.status().is_success() {
            info!("Successfully uploaded script for domain: {}", domain);
            Ok(())
        } else {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
                
            error!("Failed to upload script: {}", error_text);
            anyhow::bail!("Failed to upload script: {}", error_text)
        }
    }
    
    /// List available custom scripts
    pub async fn list_scripts(&self) -> Result<Vec<String>> {
        let endpoint = format!("{}/health", self.base_url);
        
        let response = self.client.get(&endpoint)
            .send()
            .await
            .context("Failed to get script list from browser service")?
            .json::<serde_json::Value>()
            .await
            .context("Failed to parse response")?;
            
        let scripts = response.get("custom_scripts")
            .and_then(|scripts| scripts.as_array())
            .map(|array| {
                array.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
            
        Ok(scripts)
    }
}