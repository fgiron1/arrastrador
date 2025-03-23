use anyhow::{Result, Context};
use thirtyfour::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use crate::browser::fingerprint::{FingerprintManager, CompleteFingerprint};
use crate::browser::behavior::BehaviorSimulator;
use crate::cli::config::{BrowserSettings, BrowserBehavior};
use crate::proxy::manager::ProxyManager;

/// Browser session manager
pub struct BrowserSession {
    /// Browser settings
    config: BrowserSettings,
    
    /// Fingerprint manager
    fingerprint_manager: FingerprintManager,
    
    /// Behavior simulator
    behavior_simulator: BehaviorSimulator,
    
    /// Proxy manager (optional)
    proxy_manager: Option<Arc<Mutex<ProxyManager>>>,
    
    /// WebDriver instance
    driver: Option<WebDriver>,
    
    /// Current fingerprint
    current_fingerprint: Option<CompleteFingerprint>,
}

impl BrowserSession {
    /// Create a new browser session
    pub fn new(
        config: BrowserSettings,
        proxy_manager: Option<Arc<Mutex<ProxyManager>>>,
    ) -> Self {
        let fingerprint_manager = FingerprintManager::new(config.fingerprints.clone());
        let behavior_simulator = BehaviorSimulator::new(config.behavior.clone());
        
        Self {
            config,
            fingerprint_manager,
            behavior_simulator,
            proxy_manager,
            driver: None,
            current_fingerprint: None,
        }
    }
    
    /// Initialize the browser session
    pub async fn initialize(&mut self, fingerprint_name: Option<&str>) -> Result<()> {
        // Close any existing session
        self.close().await?;
        
        // Select a fingerprint
        let fingerprint = if let Some(name) = fingerprint_name {
            self.fingerprint_manager.get_fingerprint(name)?
        } else {
            self.fingerprint_manager.random_fingerprint()?
        };
        
        // Get a proxy if available
        let proxy_config = if let Some(proxy_manager) = &self.proxy_manager {
            let mut manager = proxy_manager.lock().await;
            manager.get_proxy().await?
        } else {
            None
        };
        
        // Create WebDriver capabilities
        let mut caps = DesiredCapabilities::chrome();
        
        // Set user agent
        caps.add_chrome_arg(&format!("--user-agent={}", fingerprint.user_agent))?;
        
        // Set language
        caps.add_chrome_arg(&format!("--lang={}", fingerprint.accept_language.split(',').next().unwrap_or("en-US")))?;
        
        // Set window size
        caps.add_chrome_arg(&format!("--window-size={},{}", fingerprint.viewport.width, fingerprint.viewport.height))?;
        
        // Set headless mode if configured
        if self.config.headless {
            caps.set_headless()?;
        }
        
        // Add proxy if available
        if let Some(proxy) = proxy_config {
            match proxy.proxy_type.as_str() {
                "http" => {
                    let proxy_url = if let (Some(username), Some(password)) = (proxy.username, proxy.password) {
                        format!("http://{}:{}@{}:{}", username, password, proxy.address, proxy.port.unwrap_or(8080))
                    } else {
                        format!("http://{}:{}", proxy.address, proxy.port.unwrap_or(8080))
                    };
                    caps.add_chrome_arg(&format!("--proxy-server={}", proxy_url))?;
                },
                "socks5" => {
                    let proxy_url = if let (Some(username), Some(password)) = (proxy.username, proxy.password) {
                        format!("socks5://{}:{}@{}:{}", username, password, proxy.address, proxy.port.unwrap_or(1080))
                    } else {
                        format!("socks5://{}:{}", proxy.address, proxy.port.unwrap_or(1080))
                    };
                    caps.add_chrome_arg(&format!("--proxy-server={}", proxy_url))?;
                },
                _ => {
                    debug!("Unsupported proxy type: {}", proxy.proxy_type);
                }
            }
        }
        
        // Add additional Chrome arguments for fingerprinting protection
        caps.add_chrome_arg("--disable-blink-features=AutomationControlled")?;
        caps.add_chrome_arg("--disable-dev-shm-usage")?;
        
        // Add experimental options
        let mut experimental_options = std::collections::HashMap::new();
        experimental_options.insert("excludeSwitches", serde_json::json!(["enable-automation"]));
        experimental_options.insert("useAutomationExtension", serde_json::json!(false));
        caps.add_chrome_options(experimental_options)?;
        
        // Connect to WebDriver
        let driver = WebDriver::new("http://localhost:4444", caps).await
            .context("Failed to connect to WebDriver")?;
        
        // Set page load timeout
        driver.set_page_load_timeout(Duration::from_secs(30)).await?;
        
        debug!("Browser session initialized with fingerprint: {}", fingerprint.name);
        
        // Store the current state
        self.driver = Some(driver);
        self.current_fingerprint = Some(fingerprint);
        
        Ok(())
    }
    
    /// Navigate to a URL
    pub async fn navigate(&self, url: &str) -> Result<()> {
        let driver = self.driver.as_ref()
            .context("Browser session not initialized")?;
        
        debug!("Navigating to: {}", url);
        driver.goto(url).await
            .context(format!("Failed to navigate to URL: {}", url))?;
        
        Ok(())
    }
    
    /// Get the page source
    pub async fn get_page_source(&self) -> Result<String> {
        let driver = self.driver.as_ref()
            .context("Browser session not initialized")?;
        
        let source = driver.source().await
            .context("Failed to get page source")?;
        
        Ok(source)
    }
    
    /// Get the page title
    pub async fn get_page_title(&self) -> Result<String> {
        let driver = self.driver.as_ref()
            .context("Browser session not initialized")?;
        
        let title = driver.title().await
            .context("Failed to get page title")?;
        
        Ok(title)
    }
    
    /// Extract all links from the page
    pub async fn extract_links(&self) -> Result<Vec<String>> {
        let driver = self.driver.as_ref()
            .context("Browser session not initialized")?;
        
        let elements = driver.find_all(By::Tag("a")).await
            .context("Failed to find link elements")?;
        
        let mut links = Vec::new();
        for element in elements {
            if let Ok(href) = element.attr("href").await {
                if let Some(href) = href {
                    links.push(href);
                }
            }
        }
        
        Ok(links)
    }
    
    /// Execute JavaScript on the page
    pub async fn execute_script<T>(&self, script: &str) -> Result<T>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let driver = self.driver.as_ref()
            .context("Browser session not initialized")?;
        
        let result = driver.execute(script, Vec::new()).await
            .context("Failed to execute JavaScript")?;
        
        let value: T = result.json()
            .context("Failed to parse JavaScript result")?;
        
        Ok(value)
    }
    
    /// Simulate human-like behavior
    pub async fn simulate_human_behavior(&self) -> Result<()> {
        let driver = self.driver.as_ref()
            .context("Browser session not initialized")?;
        
        // Simulate a browsing session
        self.behavior_simulator.simulate_session(driver).await?;
        
        Ok(())
    }
    
    /// Wait for an element to be present
    pub async fn wait_for_element(&self, selector: &str, timeout_secs: u64) -> Result<WebElement> {
        let driver = self.driver.as_ref()
            .context("Browser session not initialized")?;
        
        let element = driver.query(By::Css(selector))
            .wait(Duration::from_secs(timeout_secs))
            .context(format!("Element not found: {}", selector))?
            .first()
            .await
            .context(format!("Element not found: {}", selector))?;
        
        Ok(element)
    }
    
    /// Take a screenshot
    pub async fn take_screenshot(&self, path: &str) -> Result<()> {
        let driver = self.driver.as_ref()
            .context("Browser session not initialized")?;
        
        let screenshot = driver.screenshot_as_png().await
            .context("Failed to take screenshot")?;
        
        std::fs::write(path, screenshot)
            .context(format!("Failed to save screenshot to: {}", path))?;
        
        debug!("Screenshot saved to: {}", path);
        
        Ok(())
    }
    
    /// Close the browser session
    pub async fn close(&mut self) -> Result<()> {
        if let Some(driver) = self.driver.take() {
            if let Err(e) = driver.quit().await {
                error!("Error closing browser session: {}", e);
            }
            debug!("Browser session closed");
        }
        
        self.current_fingerprint = None;
        
        Ok(())
    }
}

impl Drop for BrowserSession {
    fn drop(&mut self) {
        if let Some(driver) = self.driver.take() {
            // Spawn a task to quit the driver
            tokio::spawn(async move {
                if let Err(e) = driver.quit().await {
                    error!("Error closing browser session during drop: {}", e);
                }
            });
        }
    }
}