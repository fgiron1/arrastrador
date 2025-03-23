use anyhow::{Result, Context};
use tracing::{info, debug, warn, error};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use std::path::Path;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;
use serde::{Deserialize, Serialize};

use crate::cli::config::CrawlerConfig;
use crate::storage::queue::QueueManager;
use crate::crawler::task::{CrawlTask, TaskResult};
use crate::crawler::scheduler::Scheduler;
use crate::storage::raw::RawStorage;
use crate::storage::processed::ProcessedStorage;

/// Job status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatus {
    pub job_id: String,
    pub state: String,  // "pending", "running", "completed", "failed"
    pub url: String,
    pub pages_crawled: usize,
    pub pages_total: usize,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub errors: Vec<String>,
}

/// Main crawler controller
pub struct CrawlerController {
    config: CrawlerConfig,
    queue: Arc<QueueManager>,
    scheduler: Arc<Mutex<Scheduler>>,
    raw_storage: Arc<dyn RawStorage>,
    processed_storage: Arc<dyn ProcessedStorage>,
}

impl CrawlerController {
    /// Create a new crawler controller with the given configuration
    pub async fn new(config: CrawlerConfig) -> Result<Self> {
        // Initialize queue manager
        let queue = Arc::new(QueueManager::new(&config.storage.queue).await?);
        
        // Initialize scheduler
        let scheduler = Arc::new(Mutex::new(Scheduler::new(config.crawler.clone())));
        
        // Initialize storage
        let raw_storage = RawStorage::create(&config.storage.raw_data).await?;
        let processed_storage = ProcessedStorage::create(&config.storage.processed_data).await?;
        
        Ok(Self {
            config,
            queue,
            scheduler,
            raw_storage,
            processed_storage,
        })
    }
    
    /// Connect to an existing controller (for CLI commands)
    pub async fn connect() -> Result<Self> {
        // Load the default configuration
        let config = CrawlerConfig::load_default()?;
        
        // Connect to existing components rather than creating new ones
        let queue = Arc::new(QueueManager::connect(&config.storage.queue).await?);
        let raw_storage = RawStorage::connect(&config.storage.raw_data).await?;
        let processed_storage = ProcessedStorage::connect(&config.storage.processed_data).await?;
        
        // Create a new scheduler (stateless component)
        let scheduler = Arc::new(Mutex::new(Scheduler::new(config.crawler.clone())));
        
        Ok(Self {
            config,
            queue,
            scheduler,
            raw_storage,
            processed_storage,
        })
    }
    
    /// Start a new crawling job
    pub async fn start_job(&self, start_url: String) -> Result<String> {
        // Validate the URL
        let url = Url::parse(&start_url)
            .context(format!("Invalid URL: {}", start_url))?;
        
        // Generate a unique job ID
        let job_id = Uuid::new_v4().to_string();
        
        // Create the initial task
        let initial_task = CrawlTask {
            job_id: job_id.clone(),
            url: url.to_string(),
            depth: 0,
            parent_url: None,
            priority: 0,
        };
        
        // Create job status record
        let status = JobStatus {
            job_id: job_id.clone(),
            state: "pending".to_string(),
            url: url.to_string(),
            pages_crawled: 0,
            pages_total: 1, // Starting with 1 known URL
            started_at: Utc::now(),
            updated_at: Utc::now(),
            errors: Vec::new(),
        };
        
        // Store the job status
        self.raw_storage.store_job_status(&status).await?;
        
        // Add the initial task to the queue
        self.queue.push_task(&initial_task).await?;
        
        // Update job status to running
        let mut status = status;
        status.state = "running".to_string();
        status.updated_at = Utc::now();
        self.raw_storage.store_job_status(&status).await?;
        
        // Start worker threads if they're not already running
        // This would typically be managed by Kubernetes, but for standalone mode:
        if cfg!(feature = "standalone") {
            self.start_workers(job_id.clone()).await?;
        }
        
        Ok(job_id)
    }
    
    /// Get the status of a job
    pub async fn get_job_status(&self, job_id: &str) -> Result<JobStatus> {
        self.raw_storage.get_job_status(job_id).await
            .context(format!("Failed to get status for job: {}", job_id))
    }
    
    /// Export job data to a file
    pub async fn export_job_data(&self, job_id: &str, format: &str, output_path: &Path) -> Result<()> {
        // Get the job status to ensure it exists
        let status = self.get_job_status(job_id).await?;
        
        // Export the data based on the format
        match format {
            "json" => {
                self.processed_storage.export_as_json(job_id, output_path).await
                    .context("Failed to export data as JSON")
            },
            "csv" => {
                self.processed_storage.export_as_csv(job_id, output_path).await
                    .context("Failed to export data as CSV")
            },
            "sql" => {
                self.processed_storage.export_as_sql(job_id, output_path).await
                    .context("Failed to export data as SQL")
            },
            _ => {
                anyhow::bail!("Unsupported export format: {}", format)
            }
        }
    }
    
    /// Start worker threads for processing tasks
    #[cfg(feature = "standalone")]
    async fn start_workers(&self, job_id: String) -> Result<()> {
        use tokio::task;
        
        let worker_count = num_cpus::get().min(4); // Use at most 4 cores
        info!("Starting {} worker threads for job: {}", worker_count, job_id);
        
        for i in 0..worker_count {
            // Clone the necessary components for the worker
            let queue = self.queue.clone();
            let scheduler = self.scheduler.clone();
            let raw_storage = self.raw_storage.clone();
            let processed_storage = self.processed_storage.clone();
            let config = self.config.clone();
            let job_id = job_id.clone();
            
            // Spawn a worker task
            task::spawn(async move {
                info!("Worker {} started for job: {}", i, job_id);
                
                loop {
                    // Try to get a task from the queue
                    match queue.pop_task(&job_id).await {
                        Ok(Some(task)) => {
                            debug!("Worker {} processing task: {}", i, task.url);
                            
                            // Process the task
                            let result = Self::process_task(
                                task,
                                &config,
                                scheduler.clone(),
                                raw_storage.clone(),
                                queue.clone(),
                            ).await;
                            
                            // Handle the result
                            if let Err(e) = result {
                                error!("Worker {} task processing error: {}", i, e);
                                
                                // Update job status with error
                                if let Ok(mut status) = raw_storage.get_job_status(&job_id).await {
                                    status.errors.push(e.to_string());
                                    status.updated_at = Utc::now();
                                    if let Err(e) = raw_storage.store_job_status(&status).await {
                                        error!("Failed to update job status: {}", e);
                                    }
                                }
                            }
                        },
                        Ok(None) => {
                            // No tasks available, check if job is complete
                            if let Ok(mut status) = raw_storage.get_job_status(&job_id).await {
                                if status.pages_crawled >= status.pages_total && 
                                   queue.get_pending_count(&job_id).await.unwrap_or(1) == 0 {
                                    // Job is complete
                                    status.state = "completed".to_string();
                                    status.updated_at = Utc::now();
                                    if let Err(e) = raw_storage.store_job_status(&status).await {
                                        error!("Failed to update job status: {}", e);
                                    }
                                    
                                    info!("Worker {} detected job completion: {}", i, job_id);
                                    break;
                                }
                            }
                            
                            // Wait before checking again
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        },
                        Err(e) => {
                            error!("Worker {} queue error: {}", i, e);
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        }
                    }
                }
                
                info!("Worker {} completed for job: {}", i, job_id);
            });
        }
        
        Ok(())
    }
    
    /// Process a single crawl task
    async fn process_task(
        task: CrawlTask,
        config: &CrawlerConfig,
        scheduler: Arc<Mutex<Scheduler>>,
        raw_storage: Arc<dyn RawStorage>,
        queue: Arc<QueueManager>,
    ) -> Result<()> {
        // TODO: Implement actual crawling logic
        // This would include:
        // 1. Creating a browser instance
        // 2. Loading the URL
        // 3. Extracting content and links
        // 4. Processing and storing the results
        // 5. Scheduling new tasks for discovered links
        
        // For now, simulate processing
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Create a dummy result
        let result = TaskResult {
            job_id: task.job_id.clone(),
            url: task.url.clone(),
            depth: task.depth,
            status_code: 200,
            content_type: "text/html".to_string(),
            title: format!("Page at {}", task.url),
            links: vec!["https://example.com/1".to_string(), "https://example.com/2".to_string()],
            raw_content: "<html><body>Example content</body></html>".to_string(),
            extracted_data: serde_json::json!({"example": "data"}),
            crawled_at: Utc::now(),
        };
        
        // Store the result
        raw_storage.store_page_result(&result).await?;
        
        // Update the job status
        let mut status = raw_storage.get_job_status(&task.job_id).await?;
        status.pages_crawled += 1;
        status.updated_at = Utc::now();
        raw_storage.store_job_status(&status).await?;
        
        // Schedule new tasks for discovered links if needed
        if task.depth < config.crawler.max_depth {
            let mut scheduler = scheduler.lock().await;
            
            for link in &result.links {
                if scheduler.should_crawl(link) {
                    let new_task = CrawlTask {
                        job_id: task.job_id.clone(),
                        url: link.clone(),
                        depth: task.depth + 1,
                        parent_url: Some(task.url.clone()),
                        priority: 0,
                    };
                    
                    // Update total pages count
                    status.pages_total += 1;
                    
                    // Add task to queue
                    queue.push_task(&new_task).await?;
                }
            }
            
            // Update job status again with new total
            raw_storage.store_job_status(&status).await?;
        }
        
        Ok(())
    }
}