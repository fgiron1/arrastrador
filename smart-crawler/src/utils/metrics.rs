use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// Performance metrics collector
#[derive(Debug, Clone)]
pub struct MetricsCollector {
    /// In-memory metrics store
    metrics: Arc<Mutex<Metrics>>,
}

/// Metrics data structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metrics {
    /// Start time of the metrics collection
    pub start_time: DateTime<Utc>,
    
    /// Total requests made
    pub total_requests: usize,
    
    /// Successful requests
    pub successful_requests: usize,
    
    /// Failed requests
    pub failed_requests: usize,
    
    /// Request durations (URL -> duration in milliseconds)
    pub request_durations: HashMap<String, Vec<u64>>,
    
    /// Pages crawled per minute
    pub crawl_rate: Vec<(DateTime<Utc>, usize)>,
    
    /// Bytes downloaded
    pub bytes_downloaded: usize,
    
    /// Current requests per second
    pub current_rps: f64,
    
    /// Peak requests per second
    pub peak_rps: f64,
    
    /// HTTP status code counts
    pub status_codes: HashMap<u16, usize>,
    
    /// Custom metrics
    pub custom_metrics: HashMap<String, serde_json::Value>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        let metrics = Metrics {
            start_time: Utc::now(),
            ..Default::default()
        };
        
        Self {
            metrics: Arc::new(Mutex::new(metrics)),
        }
    }
    
    /// Record a request
    pub async fn record_request(&self, url: &str, success: bool, duration_ms: u64, status_code: Option<u16>, bytes: usize) {
        let mut metrics = self.metrics.lock().await;
        
        // Update general metrics
        metrics.total_requests += 1;
        
        if success {
            metrics.successful_requests += 1;
        } else {
            metrics.failed_requests += 1;
        }
        
        // Update bytes downloaded
        metrics.bytes_downloaded += bytes;
        
        // Record request duration
        metrics.request_durations
            .entry(url.to_string())
            .or_default()
            .push(duration_ms);
        
        // Record status code if available
        if let Some(code) = status_code {
            *metrics.status_codes.entry(code).or_default() += 1;
        }
        
        // Update crawl rate (every minute)
        let now = Utc::now();
        
        if let Some((last_time, count)) = metrics.crawl_rate.last_mut() {
            if (now - *last_time).num_seconds() < 60 {
                // Same minute, increment the count
                *count += 1;
            } else {
                // New minute, add a new entry
                metrics.crawl_rate.push((now, 1));
            }
        } else {
            // First entry
            metrics.crawl_rate.push((now, 1));
        }
        
        // Calculate current RPS
        if !metrics.crawl_rate.is_empty() {
            let (first_time, _) = metrics.crawl_rate[0];
            let elapsed_seconds = (now - first_time).num_seconds().max(1) as f64;
            metrics.current_rps = metrics.total_requests as f64 / elapsed_seconds;
            
            // Update peak RPS
            metrics.peak_rps = metrics.peak_rps.max(metrics.current_rps);
        }
    }
    
    /// Start timing a request
    pub fn start_timer(&self) -> RequestTimer {
        RequestTimer {
            start: Instant::now(),
        }
    }
    
    /// Set a custom metric
    pub async fn set_custom_metric<T: Serialize>(&self, name: &str, value: T) {
        let mut metrics = self.metrics.lock().await;
        
        if let Ok(json_value) = serde_json::to_value(value) {
            metrics.custom_metrics.insert(name.to_string(), json_value);
        }
    }
    
    /// Get all metrics
    pub async fn get_metrics(&self) -> Metrics {
        self.metrics.lock().await.clone()
    }
    
    /// Reset metrics
    pub async fn reset(&self) {
        let mut metrics = self.metrics.lock().await;
        *metrics = Metrics {
            start_time: Utc::now(),
            ..Default::default()
        };
    }
}

/// Request timer for measuring request durations
pub struct RequestTimer {
    /// Start time of the request
    start: Instant,
}

impl RequestTimer {
    /// End timing and get the duration in milliseconds
    pub fn end(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}