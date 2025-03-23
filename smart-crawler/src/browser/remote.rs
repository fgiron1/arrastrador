use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Serialize, Deserialize};
use std::time::Duration;
use tracing::{debug, error};
use url::Url;

use crate::browser::fingerprint::CompleteFingerprint;
use crate::cli::config::BrowserBehavior;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserServiceRequest {
    pub url: String,
    pub browser_type: String,
    pub fingerprint: serde_json::Value,
    pub behavior: serde_json::Value,
    pub take_screenshot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserServiceResponse {
    pub success: bool,
    pub url: String,
    pub title: String,
    pub content: String,
    pub links: Vec<String>,
    pub screenshot: Option<String>,
    pub error: Option<String>,
}

pub struct RemoteBrowserService {
    client: Client,
    base_url: String,
}

impl RemoteBrowserService {
    pub fn new() -> Self {
        // Get URL from environment variable or use default
        let base_url = std::env::var("BROWSER_SERVICE_URL")
            .unwrap_or_else(|_| "http://browser-service:5000".to_string());
            
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client");
            
        Self {
            client,
            base_url,
        }
    }
    
    pub async fn crawl_url(
        &self, 
        url: &str, 
        browser_type: &str,
        fingerprint: &CompleteFingerprint,
        behavior: &BrowserBehavior
    ) -> Result<BrowserServiceResponse> {
        let endpoint = format!("{}/crawl", self.base_url);
        
        // Convert fingerprint and behavior to JSON
        let fingerprint_json = serde_json::to_value(fingerprint)
            .context("Failed to serialize fingerprint")?;
            
        let behavior_json = serde_json::to_value(behavior)
            .context("Failed to serialize behavior")?;
            
        let request = BrowserServiceRequest {
            url: url.to_string(),
            browser_type: browser_type.to_string(),
            fingerprint: fingerprint_json,
            behavior: behavior_json,
            take_screenshot: false,
        };
        
        debug!("Sending request to browser service: {}", url);
        
        let response = self.client.post(&endpoint)
            .json(&request)
            .send()
            .await
            .context("Failed to send request to browser service")?
            .json::<BrowserServiceResponse>()
            .await
            .context("Failed to parse browser service response")?;
            
        if !response.success {
            if let Some(error) = &response.error {
                error!("Browser service error: {}", error);
                anyhow::bail!("Browser service error: {}", error);
            } else {
                anyhow::bail!("Browser service crawl failed with unknown error");
            }
        }
        
        debug!("Successfully crawled URL: {}", url);
        
        Ok(response)
    }
}