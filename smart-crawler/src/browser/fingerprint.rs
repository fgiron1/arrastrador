use std::collections::HashMap;
use anyhow::{Result, Context};
use rand::{thread_rng, Rng};
use tracing::debug;
use serde::{Serialize, Deserialize};

use crate::cli::config::BrowserFingerprint;

/// Browser fingerprint generator and manager
pub struct FingerprintManager {
    /// Available fingerprints to use
    fingerprints: Vec<BrowserFingerprint>,
}

/// Viewport dimensions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub device_scale_factor: f32,
}

/// Complete browser fingerprint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteFingerprint {
    pub name: String,
    pub user_agent: String,
    pub accept_language: String,
    pub platform: String,
    pub viewport: Viewport,
    pub headers: HashMap<String, String>,
    pub plugins: Vec<String>,
    pub fonts: Vec<String>,
    pub timezone: String,
    pub webgl_vendor: String,
    pub webgl_renderer: String,
    pub has_touch: bool,
    pub color_depth: u32,
    pub hardware_concurrency: u32,
}

impl FingerprintManager {
    /// Create a new fingerprint manager with the given fingerprints
    pub fn new(fingerprints: Vec<BrowserFingerprint>) -> Self {
        Self { fingerprints }
    }
    
    /// Select a random fingerprint
    pub fn random_fingerprint(&self) -> Result<CompleteFingerprint> {
        if self.fingerprints.is_empty() {
            anyhow::bail!("No fingerprints available");
        }
        
        let mut rng = thread_rng();
        let fingerprint = &self.fingerprints[rng.gen_range(0..self.fingerprints.len())];
        
        // Create a complete fingerprint from the basic fingerprint
        self.complete_fingerprint(fingerprint)
    }
    
    /// Get a specific fingerprint by name
    pub fn get_fingerprint(&self, name: &str) -> Result<CompleteFingerprint> {
        let fingerprint = self.fingerprints.iter()
            .find(|f| f.name == name)
            .context(format!("Fingerprint not found: {}", name))?;
        
        // Create a complete fingerprint from the basic fingerprint
        self.complete_fingerprint(fingerprint)
    }
    
    /// Complete a basic fingerprint with additional details
    fn complete_fingerprint(&self, fingerprint: &BrowserFingerprint) -> Result<CompleteFingerprint> {
        let mut rng = thread_rng();
        
        // Determine viewport based on user agent
        let viewport = if fingerprint.user_agent.contains("Mobile") {
            // Mobile viewport
            Viewport {
                width: rng.gen_range(320..480),
                height: rng.gen_range(568..812),
                device_scale_factor: rng.gen_range(1.5..3.0),
            }
        } else {
            // Desktop viewport
            Viewport {
                width: rng.gen_range(1024..1920),
                height: rng.gen_range(768..1080),
                device_scale_factor: rng.gen_range(1.0..2.0),
            }
        };
        
        // Create headers map
        let mut headers = HashMap::new();
        headers.insert("User-Agent".to_string(), fingerprint.user_agent.clone());
        headers.insert("Accept-Language".to_string(), fingerprint.accept_language.clone());
        
        // Add any extra headers from the config
        for (key, value) in &fingerprint.extra_headers {
            headers.insert(key.clone(), value.clone());
        }
        
        // Add standard headers
        headers.insert("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8".to_string());
        headers.insert("Accept-Encoding".to_string(), "gzip, deflate, br".to_string());
        headers.insert("Connection".to_string(), "keep-alive".to_string());
        headers.insert("Upgrade-Insecure-Requests".to_string(), "1".to_string());
        
        // Generate common plugins for the browser type
        let plugins = if fingerprint.user_agent.contains("Chrome") {
            vec![
                "Chrome PDF Plugin".to_string(),
                "Chrome PDF Viewer".to_string(),
                "Native Client".to_string(),
            ]
        } else if fingerprint.user_agent.contains("Firefox") {
            vec![
                "PDF Viewer".to_string(),
                "Firefox Default Browser Helper".to_string(),
            ]
        } else {
            Vec::new()
        };
        
        // Generate common fonts
        let fonts = vec![
            "Arial".to_string(),
            "Courier New".to_string(),
            "Georgia".to_string(),
            "Times New Roman".to_string(),
            "Verdana".to_string(),
        ];
        
        // Generate WebGL info based on platform
        let (webgl_vendor, webgl_renderer) = if fingerprint.platform.contains("Win") {
            (
                "Google Inc.".to_string(),
                "ANGLE (Intel(R) HD Graphics Direct3D11 vs_5_0 ps_5_0)".to_string(),
            )
        } else if fingerprint.platform.contains("Mac") {
            (
                "Apple Inc.".to_string(),
                "Apple GPU".to_string(),
            )
        } else {
            (
                "Mesa".to_string(),
                "Mesa DRI Intel(R) HD Graphics 620 (Kaby Lake GT2)".to_string(),
            )
        };
        
        // Create the complete fingerprint
        let complete = CompleteFingerprint {
            name: fingerprint.name.clone(),
            user_agent: fingerprint.user_agent.clone(),
            accept_language: fingerprint.accept_language.clone(),
            platform: fingerprint.platform.clone(),
            viewport,
            headers,
            plugins,
            fonts,
            timezone: "America/New_York".to_string(), // Could randomize this
            webgl_vendor,
            webgl_renderer,
            has_touch: fingerprint.user_agent.contains("Mobile"),
            color_depth: 24,
            hardware_concurrency: rng.gen_range(2..8),
        };
        
        debug!("Generated fingerprint: {}", complete.name);
        
        Ok(complete)
    }
}