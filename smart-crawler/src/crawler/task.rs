use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use serde_json::Value;

/// Represents a crawling task to be executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlTask {
    /// Unique identifier for the job this task belongs to
    pub job_id: String,
    
    /// URL to crawl
    pub url: String,
    
    /// Current depth in the crawl tree (0 for seed URLs)
    pub depth: u32,
    
    /// Parent URL that led to this URL (None for seed URLs)
    pub parent_url: Option<String>,
    
    /// Priority of this task (higher values = higher priority)
    pub priority: i32,
}

/// Result of a completed crawl task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Unique identifier for the job this result belongs to
    pub job_id: String,
    
    /// URL that was crawled
    pub url: String,
    
    /// Depth at which this URL was crawled
    pub depth: u32,
    
    /// HTTP status code
    pub status_code: u16,
    
    /// Content type of the response
    pub content_type: String,
    
    /// Page title (if available)
    pub title: String,
    
    /// Links discovered on the page
    pub links: Vec<String>,
    
    /// Raw content of the page
    pub raw_content: String,
    
    /// Structured data extracted from the page
    pub extracted_data: Value,
    
    /// Timestamp when the page was crawled
    pub crawled_at: DateTime<Utc>,
}

/// Error result from a crawl task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskError {
    /// Unique identifier for the job this error belongs to
    pub job_id: String,
    
    /// URL that was being crawled
    pub url: String,
    
    /// Error message
    pub error: String,
    
    /// Error type (e.g., "network", "timeout", "parsing")
    pub error_type: String,
    
    /// Timestamp when the error occurred
    pub occurred_at: DateTime<Utc>,
}