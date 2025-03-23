use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use tracing::{info, debug, error};
use std::collections::HashMap;

/// Main configuration structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CrawlerConfig {
    pub crawler: CrawlerSettings,
    pub browser: BrowserSettings,
    pub proxy: ProxySettings,
    pub storage: StorageSettings,
    pub browser_service: BrowserServiceSettings,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BrowserServiceSettings {
    pub enabled: bool,
    pub url: String,
}


/// Crawler-specific settings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CrawlerSettings {
    pub max_depth: u32,
    pub max_pages: u32,
    pub politeness_delay: u64,  // Delay between requests in milliseconds
    pub respect_robots_txt: bool,
    pub allowed_domains: Vec<String>,
    pub url_patterns: UrlPatterns,
    pub user_agent: String,
}

/// URL pattern settings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UrlPatterns {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

/// Browser simulation settings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BrowserSettings {
    pub browser_type: String,  // "chrome", "firefox", etc.
    pub headless: bool,
    pub viewport: Viewport,
    pub fingerprints: Vec<BrowserFingerprint>,
    pub behavior: BrowserBehavior,
}

/// Browser viewport settings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub device_scale_factor: f32,
}

/// Browser fingerprint settings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BrowserFingerprint {
    pub name: String,
    pub user_agent: String,
    pub accept_language: String,
    pub platform: String,
    pub extra_headers: HashMap<String, String>,
}

/// Browser behavior simulation settings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BrowserBehavior {
    pub scroll_behavior: String,  // "random", "smooth", "none"
    pub click_delay: (u64, u64),  // Min and max delay in milliseconds
    pub typing_speed: (u64, u64), // Min and max milliseconds per character
    pub mouse_movement: bool,
    pub session_duration: (u64, u64), // Min and max session duration in seconds
}

/// Proxy settings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProxySettings {
    pub enabled: bool,
    pub rotation_strategy: String, // "session", "request", "timed"
    pub rotation_interval: Option<u64>, // Seconds between rotations if using "timed"
    pub proxy_list: Vec<ProxyConfig>,
}

/// Individual proxy configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProxyConfig {
    pub name: String,
    pub proxy_type: String, // "http", "socks5", "vpn"
    pub address: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub country: Option<String>,
}

/// Storage settings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StorageSettings {
    pub queue: QueueSettings,
    pub raw_data: RawDataSettings,
    pub processed_data: ProcessedDataSettings,
}

/// Queue settings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueueSettings {
    pub redis_url: String,
    pub task_ttl: u64, // Time to live for tasks in seconds
}

/// Raw data storage settings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RawDataSettings {
    pub storage_type: String, // "mongodb", "filesystem"
    pub connection_string: String,
    pub database_name: String,
    pub collection_prefix: String,
}

/// Processed data storage settings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProcessedDataSettings {
    pub storage_type: String, // "postgresql", "sqlite", "filesystem"
    pub connection_string: String,
    pub schema_name: String,
    pub table_prefix: String,
}

impl Default for CrawlerConfig {
    fn default() -> Self {
        Self {
            crawler: CrawlerSettings {
                max_depth: 3,
                max_pages: 1000,
                politeness_delay: 2000,
                respect_robots_txt: true,
                allowed_domains: vec![],
                url_patterns: UrlPatterns {
                    include: vec![],
                    exclude: vec![],
                },
                user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36".to_string(),
            },
            browser: BrowserSettings {
                browser_type: "chrome".to_string(),
                headless: true,
                viewport: Viewport {
                    width: 1920,
                    height: 1080,
                    device_scale_factor: 1.0,
                },
                fingerprints: vec![
                    BrowserFingerprint {
                        name: "windows_chrome".to_string(),
                        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36".to_string(),
                        accept_language: "en-US,en;q=0.9".to_string(),
                        platform: "Win32".to_string(),
                        extra_headers: HashMap::new(),
                    },
                ],
                behavior: BrowserBehavior {
                    scroll_behavior: "random".to_string(),
                    click_delay: (100, 300),
                    typing_speed: (50, 150),
                    mouse_movement: true,
                    session_duration: (300, 1800),
                },
            },
            proxy: ProxySettings {
                enabled: false,
                rotation_strategy: "session".to_string(),
                rotation_interval: Some(600),
                proxy_list: vec![],
            },
            storage: StorageSettings {
                queue: QueueSettings {
                    redis_url: "redis://localhost:6379".to_string(),
                    task_ttl: 86400,
                },
                raw_data: RawDataSettings {
                    storage_type: "mongodb".to_string(),
                    connection_string: "mongodb://localhost:27017".to_string(),
                    database_name: "crawler".to_string(),
                    collection_prefix: "raw".to_string(),
                },
                processed_data: ProcessedDataSettings {
                    storage_type: "postgresql".to_string(),
                    connection_string: "postgresql://postgres:postgres@localhost:5432/crawler".to_string(),
                    schema_name: "public".to_string(),
                    table_prefix: "crawled".to_string(),
                },
            },
            browser_service: BrowserServiceSettings {
                 enabled: true,
                 url: "http://localhost:5000".to_string(), 
            }
        }
    }
}

impl CrawlerConfig {
    /// Get the path to the config directory
    fn config_dir() -> PathBuf {
        let mut path = if let Some(proj_dirs) = directories::ProjectDirs::from("com", "smart-crawler", "smart-crawler") {
            proj_dirs.config_dir().to_path_buf()
        } else {
            PathBuf::from("./config")
        };
        
        // Create the sites directory if it doesn't exist
        path.push("sites");
        if !path.exists() {
            if let Err(e) = fs::create_dir_all(&path) {
                error!("Failed to create config directory: {}", e);
            }
        }
        
        // Move back up to the config directory
        path.pop();
        path
    }
    
    /// Load the default configuration
    pub fn load_default() -> Result<Self> {
        let config_dir = Self::config_dir();
        let config_path = config_dir.join("default.yaml");
        
        if config_path.exists() {
            Self::load_from_file(&config_path)
        } else {
            // Create and save the default configuration
            info!("Default configuration not found. Creating...");
            let config = Self::default();
            config.save_as_default()?;
            Ok(config)
        }
    }
    
    /// Load a configuration profile
    pub fn load_profile(profile: &str) -> Result<Self> {
        let config_dir = Self::config_dir();
        let profile_path = config_dir.join("sites").join(format!("{}.yaml", profile));
        
        if profile_path.exists() {
            Self::load_from_file(&profile_path)
        } else {
            anyhow::bail!("Profile '{}' not found", profile)
        }
    }
    
    /// Load configuration from a file
    fn load_from_file(path: &Path) -> Result<Self> {
        debug!("Loading configuration from: {}", path.display());
        let contents = fs::read_to_string(path)
            .context(format!("Failed to read configuration file: {}", path.display()))?;
        
        let config: Self = serde_yaml::from_str(&contents)
            .context(format!("Failed to parse configuration file: {}", path.display()))?;
        
        Ok(config)
    }
    
    /// Save the configuration as the default
    pub fn save_as_default(&self) -> Result<()> {
        let config_dir = Self::config_dir();
        let config_path = config_dir.join("default.yaml");
        
        self.save_to_file(&config_path)
    }
    
    /// Save the configuration as a profile
    pub async fn save_as_profile(&self, profile: &str) -> Result<()> {
        let config_dir = Self::config_dir();
        let sites_dir = config_dir.join("sites");
        
        // Create the sites directory if it doesn't exist
        if !sites_dir.exists() {
            fs::create_dir_all(&sites_dir)
                .context(format!("Failed to create sites directory: {}", sites_dir.display()))?;
        }
        
        let profile_path = sites_dir.join(format!("{}.yaml", profile));
        self.save_to_file(&profile_path)
    }
    
    /// Save the configuration to a file
    fn save_to_file(&self, path: &Path) -> Result<()> {
        debug!("Saving configuration to: {}", path.display());
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .context(format!("Failed to create directory: {}", parent.display()))?;
            }
        }
        
        let contents = serde_yaml::to_string(self)
            .context("Failed to serialize configuration")?;
        
        fs::write(path, contents)
            .context(format!("Failed to write configuration file: {}", path.display()))?;
        
        Ok(())
    }
    
    /// List all available profiles
    pub async fn list_profiles() -> Result<Vec<String>> {
        let config_dir = Self::config_dir();
        let sites_dir = config_dir.join("sites");
        
        if !sites_dir.exists() {
            return Ok(vec![]);
        }
        
        let mut profiles = Vec::new();
        
        for entry in fs::read_dir(sites_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "yaml") {
                if let Some(stem) = path.file_stem() {
                    if let Some(name) = stem.to_str() {
                        profiles.push(name.to_string());
                    }
                }
            }
        }
        
        Ok(profiles)
    }
}