use anyhow::{Result, Context};
use rand::{thread_rng, Rng};
use tokio::time::{Instant, Duration};
use tracing::{debug, warn, error};
use reqwest::Client;
use std::collections::HashMap;

use crate::cli::config::{ProxySettings, ProxyConfig};

/// Proxy rotation and management
pub struct ProxyManager {
    /// Proxy configuration
    config: ProxySettings,
    
    /// Currently active proxy
    current_proxy: Option<ProxyConfig>,
    
    /// Last rotation time
    last_rotation: Instant,
    
    /// Proxy status map (address -> working status)
    proxy_status: HashMap<String, bool>,
}

impl ProxyManager {
    /// Create a new proxy manager
    pub fn new(config: ProxySettings) -> Self {
        Self {
            config,
            current_proxy: None,
            last_rotation: Instant::now(),
            proxy_status: HashMap::new(),
        }
    }
    
    /// Get a proxy for use
    pub async fn get_proxy(&mut self) -> Result<Option<ProxyConfig>> {
        // If proxies are disabled, return None
        if !self.config.enabled {
            return Ok(None);
        }
        
        // Check if we need to rotate based on the strategy
        let should_rotate = match self.config.rotation_strategy.as_str() {
            "request" => true,
            "timed" => {
                if let Some(interval) = self.config.rotation_interval {
                    self.last_rotation.elapsed() >= Duration::from_secs(interval)
                } else {
                    // Default to 10 minutes if not specified
                    self.last_rotation.elapsed() >= Duration::from_secs(600)
                }
            },
            "session" => self.current_proxy.is_none(),
            _ => true,
        };
        
        if should_rotate || self.current_proxy.is_none() {
            self.rotate_proxy().await?;
        }
        
        Ok(self.current_proxy.clone())
    }
    
    /// Rotate to a new proxy
    pub async fn rotate_proxy(&mut self) -> Result<()> {
        if self.config.proxy_list.is_empty() {
            anyhow::bail!("No proxies configured");
        }
        
        // Get a list of working proxies (or all if none have been tested)
        let working_proxies: Vec<&ProxyConfig> = if self.proxy_status.is_empty() {
            self.config.proxy_list.iter().collect()
        } else {
            self.config.proxy_list.iter()
                .filter(|p| *self.proxy_status.get(&p.address).unwrap_or(&true))
                .collect()
        };
        
        if working_proxies.is_empty() {
            // If no working proxies, reset and try again
            debug!("No working proxies found, resetting status");
            self.proxy_status.clear();
            return self.rotate_proxy().await;
        }
        
        // Select a random proxy
        let mut rng = thread_rng();
        let new_proxy = working_proxies[rng.gen_range(0..working_proxies.len())].clone();
        
        debug!("Rotated to proxy: {}", new_proxy.name);
        
        self.current_proxy = Some(new_proxy);
        self.last_rotation = Instant::now();
        
        Ok(())
    }
    
    /// Mark the current proxy as failed
    pub async fn mark_current_failed(&mut self) -> Result<()> {
        if let Some(proxy) = &self.current_proxy {
            debug!("Marking proxy as failed: {}", proxy.name);
            self.proxy_status.insert(proxy.address.clone(), false);
            self.rotate_proxy().await?;
        }
        
        Ok(())
    }
    
    /// Test all proxies and update their status
    pub async fn test_all_proxies(&mut self) -> Result<()> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .context("Failed to create HTTP client")?;
        
        for proxy in &self.config.proxy_list {
            let working = self.test_proxy(&client, proxy).await;
            self.proxy_status.insert(proxy.address.clone(), working);
            
            if working {
                debug!("Proxy tested OK: {}", proxy.name);
            } else {
                warn!("Proxy test failed: {}", proxy.name);
            }
        }
        
        Ok(())
    }
    
    /// Test a single proxy
    async fn test_proxy(&self, client: &Client, proxy: &ProxyConfig) -> bool {
        // Build the proxy URL
        let proxy_url = match proxy.proxy_type.as_str() {
            "http" => {
                if let (Some(username), Some(password)) = (&proxy.username, &proxy.password) {
                    format!("http://{}:{}@{}:{}", username, password, proxy.address, proxy.port.unwrap_or(8080))
                } else {
                    format!("http://{}:{}", proxy.address, proxy.port.unwrap_or(8080))
                }
            },
            "socks5" => {
                if let (Some(username), Some(password)) = (&proxy.username, &proxy.password) {
                    format!("socks5://{}:{}@{}:{}", username, password, proxy.address, proxy.port.unwrap_or(1080))
                } else {
                    format!("socks5://{}:{}", proxy.address, proxy.port.unwrap_or(1080))
                }
            },
            _ => {
                error!("Unsupported proxy type: {}", proxy.proxy_type);
                return false;
            }
        };
        
        // Create a proxy-specific client
        let proxy_client = match reqwest::Proxy::all(&proxy_url) {
            Ok(proxy) => {
                match client.clone().proxy(proxy).build() {
                    Ok(client) => client,
                    Err(e) => {
                        error!("Failed to create proxy client: {}", e);
                        return false;
                    }
                }
            },
            Err(e) => {
                error!("Invalid proxy URL {}: {}", proxy_url, e);
                return false;
            }
        };
        
        // Test the proxy by making a request to a reliable endpoint
        match proxy_client.get("https://www.google.com").send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
}