use anyhow::{Result, Context};
use tokio::process::Command;
use std::path::{Path, PathBuf};
use std::fs;
use tracing::{debug, error, info};
use rand::{thread_rng, Rng};

/// VPN connection manager
pub struct VpnManager {
    /// Directory containing VPN profiles
    profiles_dir: PathBuf,
    
    /// Currently active profile
    active_profile: Option<String>,
}

impl VpnManager {
    /// Create a new VPN manager
    pub fn new<P: AsRef<Path>>(profiles_dir: P) -> Self {
        Self {
            profiles_dir: PathBuf::from(profiles_dir.as_ref()),
            active_profile: None,
        }
    }
    
    /// List available VPN profiles
    pub fn list_profiles(&self) -> Result<Vec<String>> {
        let mut profiles = Vec::new();
        
        for entry in fs::read_dir(&self.profiles_dir)
            .context(format!("Failed to read profiles directory: {}", self.profiles_dir.display()))? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "ovpn") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    profiles.push(name.to_string());
                }
            }
        }
        
        Ok(profiles)
    }
    
    /// Connect to a VPN profile
    pub async fn connect(&mut self, profile_name: &str) -> Result<()> {
        // Disconnect from any active VPN first
        self.disconnect().await?;
        
        let profile_path = self.profiles_dir.join(format!("{}.ovpn", profile_name));
        
        if !profile_path.exists() {
            anyhow::bail!("VPN profile not found: {}", profile_name);
        }
        
        // Connect to the VPN
        debug!("Connecting to VPN: {}", profile_name);
        
        #[cfg(target_os = "linux")]
        {
            let output = Command::new("sudo")
                .arg("openvpn")
                .arg("--config")
                .arg(&profile_path)
                .arg("--daemon")
                .output()
                .await
                .context("Failed to start OpenVPN")?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Failed to connect to VPN: {}", stderr);
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            let output = Command::new("sudo")
                .arg("openvpn")
                .arg("--config")
                .arg(&profile_path)
                .arg("--daemon")
                .output()
                .await
                .context("Failed to start OpenVPN")?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Failed to connect to VPN: {}", stderr);
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            let output = Command::new("cmd")
                .arg("/c")
                .arg("start")
                .arg("/b")
                .arg("openvpn")
                .arg("--config")
                .arg(&profile_path)
                .output()
                .await
                .context("Failed to start OpenVPN")?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Failed to connect to VPN: {}", stderr);
            }
        }
        
        // Store the active profile
        self.active_profile = Some(profile_name.to_string());
        info!("Connected to VPN: {}", profile_name);
        
        Ok(())
    }
    
    /// Disconnect from the VPN
    pub async fn disconnect(&mut self) -> Result<()> {
        if self.active_profile.is_none() {
            return Ok(());
        }
        
        debug!("Disconnecting from VPN");
        
        #[cfg(target_os = "linux")]
        {
            let output = Command::new("sudo")
                .arg("killall")
                .arg("-SIGINT")
                .arg("openvpn")
                .output()
                .await
                .context("Failed to stop OpenVPN")?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                error!("Failed to disconnect from VPN: {}", stderr);
                // Continue anyway
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            let output = Command::new("sudo")
                .arg("killall")
                .arg("-SIGINT")
                .arg("openvpn")
                .output()
                .await
                .context("Failed to stop OpenVPN")?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                error!("Failed to disconnect from VPN: {}", stderr);
                // Continue anyway
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            let output = Command::new("taskkill")
                .arg("/F")
                .arg("/IM")
                .arg("openvpn.exe")
                .output()
                .await
                .context("Failed to stop OpenVPN")?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                error!("Failed to disconnect from VPN: {}", stderr);
                // Continue anyway
            }
        }
        
        // Clear the active profile
        let previous = self.active_profile.take();
        debug!("Disconnected from VPN: {:?}", previous);
        
        // Give the system a moment to clean up
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        Ok(())
    }
    
    /// Connect to a random VPN profile
    pub async fn connect_random(&mut self) -> Result<String> {
        let profiles = self.list_profiles()?;
        
        if profiles.is_empty() {
            anyhow::bail!("No VPN profiles available");
        }
        
        // Select a random profile
        let mut rng = thread_rng();
        let profile = profiles[rng.gen_range(0..profiles.len())].clone();
        
        // Connect to the profile
        self.connect(&profile).await?;
        
        Ok(profile)
    }
    
    /// Check if connected to a VPN
    pub async fn is_connected(&self) -> bool {
        self.active_profile.is_some()
    }
    
    /// Get the currently active profile name
    pub fn get_active_profile(&self) -> Option<&str> {
        self.active_profile.as_deref()
    }
}

impl Drop for VpnManager {
    fn drop(&mut self) {
        if let Some(profile) = &self.active_profile {
            debug!("Disconnecting from VPN on drop: {}", profile);
            
            // Spawn a blocking task to disconnect
            let future = async {
                #[cfg(target_os = "linux")]
                {
                    let _ = Command::new("sudo")
                        .arg("killall")
                        .arg("-SIGINT")
                        .arg("openvpn")
                        .output()
                        .await;
                }
                
                #[cfg(target_os = "macos")]
                {
                    let _ = Command::new("sudo")
                        .arg("killall")
                        .arg("-SIGINT")
                        .arg("openvpn")
                        .output()
                        .await;
                }
                
                #[cfg(target_os = "windows")]
                {
                    let _ = Command::new("taskkill")
                        .arg("/F")
                        .arg("/IM")
                        .arg("openvpn.exe")
                        .output()
                        .await;
                }
            };
            
            // Spawn the task
            tokio::task::spawn(future);
        }
    }
}