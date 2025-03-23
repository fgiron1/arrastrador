use anyhow::Result;
use rand::{thread_rng, Rng};
use std::time::Duration;
use tokio::time::sleep;
use thirtyfour::prelude::*;
use tracing::debug;

use crate::cli::config::BrowserBehavior;

/// Human-like behavior simulator for browser automation
pub struct BehaviorSimulator {
    /// Configuration for behavior simulation
    config: BrowserBehavior,
}

impl BehaviorSimulator {
    /// Create a new behavior simulator with the given configuration
    pub fn new(config: BrowserBehavior) -> Self {
        Self { config }
    }
    
    /// Simulate human-like scrolling
    pub async fn scroll(&self, driver: &WebDriver, distance: Option<i32>) -> Result<()> {
        let mut rng = thread_rng();
        
        // Determine scroll distance if not specified
        let distance = distance.unwrap_or_else(|| {
            // Random scroll distance between 100 and 800 pixels
            rng.gen_range(100..800)
        });
        
        // Determine scroll speed based on configuration
        let scroll_behavior = match self.config.scroll_behavior.as_str() {
            "smooth" => "smooth",
            "random" => if rng.gen_bool(0.7) { "smooth" } else { "auto" },
            _ => "auto",
        };
        
        // Execute scroll with random pauses
        let mut scrolled = 0;
        while scrolled < distance {
            // Random chunk size for each scroll action
            let chunk = rng.gen_range(100..300).min(distance - scrolled);
            scrolled += chunk;
            
            // Execute scroll
            let script = format!(
                "window.scrollBy({{ top: {}, left: 0, behavior: '{}' }});",
                chunk, scroll_behavior
            );
            driver.execute(&script, Vec::new()).await?;
            
            // Random pause between scroll actions
            let pause_ms = rng.gen_range(300..800);
            sleep(Duration::from_millis(pause_ms)).await;
        }
        
        debug!("Scrolled {} pixels", distance);
        
        Ok(())
    }
    
    /// Simulate human-like clicking with a random delay
    pub async fn click(&self, element: &WebElement) -> Result<()> {
        let mut rng = thread_rng();
        
        // Random delay before clicking (simulates human reaction time)
        let delay_ms = rng.gen_range(self.config.click_delay.0..self.config.click_delay.1);
        sleep(Duration::from_millis(delay_ms)).await;
        
        // If configured, simulate mouse movement
        if self.config.mouse_movement {
            // Move mouse to a random point in the element
            let size = element.rect().await?;
            let offset_x = rng.gen_range(5..(size.width as i32 - 5));
            let offset_y = rng.gen_range(5..(size.height as i32 - 5));
            
            element.scroll_into_view().await?;
            element.move_to(offset_x, offset_y).await?;
            
            // Small delay after mouse movement
            sleep(Duration::from_millis(rng.gen_range(50..150))).await;
        }
        
        // Click the element
        element.click().await?;
        
        debug!("Clicked element");
        
        Ok(())
    }
    
    /// Simulate human-like typing with variable speed
    pub async fn type_text(&self, element: &WebElement, text: &str) -> Result<()> {
        let mut rng = thread_rng();
        
        // Clear the field first
        element.clear().await?;
        
        // Type each character with a random delay
        for c in text.chars() {
            let delay_ms = rng.gen_range(self.config.typing_speed.0..self.config.typing_speed.1);
            element.send_keys(c.to_string()).await?;
            sleep(Duration::from_millis(delay_ms)).await;
        }
        
        debug!("Typed text: {}", text);
        
        Ok(())
    }
    
    /// Simulate random pauses during browsing
    pub async fn random_pause(&self) -> Result<()> {
        let mut rng = thread_rng();
        
        // Generate a random pause duration with weighted distribution
        // 80% chance of short pause, 15% chance of medium pause, 5% chance of long pause
        let pause_ms = if rng.gen_bool(0.8) {
            // Short pause (0.5 - 2 seconds)
            rng.gen_range(500..2000)
        } else if rng.gen_bool(0.75) {
            // Medium pause (2 - 5 seconds)
            rng.gen_range(2000..5000)
        } else {
            // Long pause (5 - 10 seconds)
            rng.gen_range(5000..10000)
        };
        
        sleep(Duration::from_millis(pause_ms)).await;
        debug!("Paused for {} ms", pause_ms);
        
        Ok(())
    }
    
    /// Simulate browsing session behavior
    pub async fn simulate_session(&self, driver: &WebDriver) -> Result<()> {
        let mut rng = thread_rng();
        
        // Determine session duration
        let session_seconds = rng.gen_range(
            self.config.session_duration.0..self.config.session_duration.1
        );
        
        debug!("Starting simulated browsing session for {} seconds", session_seconds);
        
        // Initialize session timer
        let start_time = std::time::Instant::now();
        
        // Continue browsing until session time expires
        while start_time.elapsed().as_secs() < session_seconds {
            // Decide on a random action
            let action = rng.gen_range(0..5);
            
            match action {
                0 => {
                    // Scroll down
                    self.scroll(driver, None).await?;
                },
                1 => {
                    // Click a random link if available
                    if let Ok(links) = driver.find_all(By::Tag("a")).await {
                        if !links.is_empty() {
                            let link = &links[rng.gen_range(0..links.len())];
                            if let Ok(visible) = link.is_displayed().await {
                                if visible {
                                    self.click(link).await?;
                                    // Wait for page load
                                    sleep(Duration::from_secs(2)).await;
                                    continue;
                                }
                            }
                        }
                    }
                    // If no links clicked, scroll instead
                    self.scroll(driver, None).await?;
                },
                2 => {
                    // Pause
                    self.random_pause().await?;
                },
                3 => {
                    // Check for and interact with a form input
                    if let Ok(inputs) = driver.find_all(By::Css("input[type='text'], input[type='search']")).await {
                        if !inputs.is_empty() {
                            let input = &inputs[rng.gen_range(0..inputs.len())];
                            if let Ok(visible) = input.is_displayed().await {
                                if visible {
                                    static SAMPLE_SEARCHES: &[&str] = &[
                                        "product information",
                                        "how to",
                                        "best price",
                                        "review",
                                        "compare",
                                    ];
                                    let search_text = SAMPLE_SEARCHES[rng.gen_range(0..SAMPLE_SEARCHES.len())];
                                    self.type_text(input, search_text).await?;
                                    
                                    // Try to find and click a submit button
                                    if let Ok(buttons) = driver.find_all(By::Css("button[type='submit'], input[type='submit']")).await {
                                        if !buttons.is_empty() {
                                            let button = &buttons[0];
                                            self.click(button).await?;
                                            // Wait for results
                                            sleep(Duration::from_secs(2)).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                4 => {
                    // Go back in history occasionally
                    if rng.gen_bool(0.3) {
                        driver.back().await?;
                        sleep(Duration::from_secs(1)).await;
                    } else {
                        // Or scroll up
                        self.scroll(driver, Some(-500)).await?;
                    }
                },
                _ => {
                    // Default action, just scroll
                    self.scroll(driver, None).await?;
                },
            }
        }
        
        debug!("Completed simulated browsing session after {} seconds", session_seconds);
        
        Ok(())
    }
}