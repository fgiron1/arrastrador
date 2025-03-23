use anyhow::{Result, Context};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use url::Url;
use uuid::Uuid;

use crate::browser::fingerprint::FingerprintManager;
use crate::browser::remote::RemoteBrowserService;
use crate::cli::config::CrawlerConfig;
use crate::crawler::scheduler::Scheduler;
use crate::crawler::task::{CrawlTask, TaskResult};
use crate::storage::queue::QueueManager;
use crate::storage::raw::{RawStorage, RawStorageBackend, JobStatus};
use crate::storage::processed::{ProcessedStorage, ProcessedStorageFactory};

pub struct CrawlerController {
    config: CrawlerConfig,
    queue: Arc<QueueManager>,
    scheduler: Arc<Mutex<Scheduler>>,
    raw_storage: Arc<dyn RawStorageBackend>,
    processed_storage: Arc<dyn ProcessedStorage>,
    browser_service: Arc<RemoteBrowserService>,
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
        let processed_storage = ProcessedStorageFactory::create(&config.storage.processed_data).await?;        
        // Initialize browser service
        let browser_service = Arc::new(RemoteBrowserService::new());
        
        Ok(Self {
            config,
            queue,
            scheduler,
            raw_storage,
            processed_storage,
            browser_service,
        })
    }
    
    // Connect to an existing controller
    pub async fn connect() -> Result<Self> {
        // Load the default configuration
        let config = CrawlerConfig::load_default()?;
        
        // Connect to existing components rather than creating new ones
        let queue = Arc::new(QueueManager::connect(&config.storage.queue).await?);
        let raw_storage = RawStorage::connect(&config.storage.raw_data).await?;
        let processed_storage = ProcessedStorageFactory::connect(&config.storage.processed_data).await?;
        
        // Create a new scheduler (stateless component)
        let scheduler = Arc::new(Mutex::new(Scheduler::new(config.crawler.clone())));
        
        // Initialize browser service
        let browser_service = Arc::new(RemoteBrowserService::new());
        
        Ok(Self {
            config,
            queue,
            scheduler,
            raw_storage,
            processed_storage,
            browser_service,
        })
    }
    
    /// Start a new crawling job
    pub async fn start_job(&self, seed_url: String) -> Result<String> {
        // Generate a unique job ID
        let job_id = Uuid::new_v4().to_string();
        
        // Create the initial job status
        let status = JobStatus {
            job_id: job_id.clone(),
            seed_url: seed_url.clone(),
            state: "pending".to_string(),
            pages_crawled: 0,
            pages_total: 1,  // Start with the seed URL
            started_at: Utc::now(),
            updated_at: Utc::now(),
            errors: Vec::new(),
        };
        
        // Store the job status
        self.raw_storage.store_job_status(&status).await?;
        
        // Create the initial task
        let task = CrawlTask {
            job_id: job_id.clone(),
            url: seed_url,
            depth: 0,
            parent_url: None,
            priority: 0,
        };
        
        // Add the task to the queue
        self.queue.push_task(&task).await?;
        
        // Start worker threads if in standalone mode
        #[cfg(feature = "standalone")]
        self.start_workers(job_id.clone()).await?;
        
        // Update job status to running
        let mut updated_status = status;
        updated_status.state = "running".to_string();
        self.raw_storage.store_job_status(&updated_status).await?;
        
        Ok(job_id)
    }
    
    /// Get the status of a job
    pub async fn get_job_status(&self, job_id: &str) -> Result<JobStatus> {
        self.raw_storage.get_job_status(job_id).await
    }
    
    /// Export job data
    pub async fn export_job_data(&self, job_id: &str, format: &str, output_path: &std::path::Path) -> Result<()> {
        match format {
            "json" => {
                self.processed_storage.export_as_json(job_id, output_path).await?;
            },
            "csv" => {
                self.processed_storage.export_as_csv(job_id, output_path).await?;
            },
            "sql" => {
                self.processed_storage.export_as_sql(job_id, output_path).await?;
            },
            _ => {
                anyhow::bail!("Unsupported export format: {}", format);
            }
        }
        
        Ok(())
    }
    
    /// Process a crawl task
    async fn process_task(
        task: CrawlTask,
        config: &CrawlerConfig,
        scheduler: Arc<Mutex<Scheduler>>,
        raw_storage: Arc<dyn RawStorageBackend>,
        queue: Arc<QueueManager>,
        browser_service: Arc<RemoteBrowserService>,
    ) -> Result<()> {
        // Get fingerprint
        let fingerprint_manager = FingerprintManager::new(config.browser.fingerprints.clone());
        let fingerprint = fingerprint_manager.random_fingerprint()?;
        
        // Crawl the URL using the remote browser service
        let response = browser_service.crawl_url(
            &task.url,
            &config.browser.browser_type,
            &fingerprint,
            &config.browser.behavior
        ).await?;
        
        // Parse the URL to get absolute links
        let base_url = Url::parse(&task.url)?;
        
        // Process links to get absolute URLs
        let links: Vec<String> = response.links.iter()
            .filter_map(|link| {
                match Url::parse(link) {
                    Ok(absolute_url) => Some(absolute_url.to_string()),
                    Err(_) => {
                        // Try to resolve relative URL
                        base_url.join(link).ok().map(|u| u.to_string())
                    }
                }
            })
            .collect();
        
        // Create a task result
        let result = TaskResult {
            job_id: task.job_id.clone(),
            url: task.url.clone(),
            depth: task.depth,
            status_code: 200, // We assume success since the service returned success
            content_type: "text/html".to_string(),
            title: response.title,
            links,
            raw_content: response.content,
            extracted_data: serde_json::json!({}),
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
            let mut scheduler_lock = scheduler.lock().await;
            
            for link in &result.links {
                if scheduler_lock.should_crawl(link) {
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
    
    // Start worker threads in standalone mode
    #[cfg(feature = "standalone")]
    async fn start_workers(&self, job_id: String) -> Result<()> {
        use tokio::task;
        use num_cpus;
        
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
            let browser_service = self.browser_service.clone();
            
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
                                task.clone(),
                                &config,
                                scheduler.clone(),
                                raw_storage.clone(),
                                queue.clone(),
                                browser_service.clone(),
                            ).await;
                            
                            // Handle the result
                            match result {
                                Ok(_) => {
                                    // Mark the task as complete
                                    if let Err(e) = queue.complete_task(&job_id, &task.url).await {
                                        error!("Failed to mark task as complete: {}", e);
                                    }
                                },
                                Err(e) => {
                                    error!("Worker {} task processing error: {}", i, e);
                                    
                                    // Mark the task as failed
                                    if let Err(e) = queue.fail_task(&job_id, &task.url, &e.to_string()).await {
                                        error!("Failed to mark task as failed: {}", e);
                                    }
                                    
                                    // Update job status with error
                                    if let Ok(mut status) = raw_storage.get_job_status(&job_id).await {
                                        status.errors.push(e.to_string());
                                        status.updated_at = Utc::now();
                                        if let Err(e) = raw_storage.store_job_status(&status).await {
                                            error!("Failed to update job status: {}", e);
                                        }
                                    }
                                }
                            }
                        },
                        Ok(None) => {
                            // No tasks available, check if we're done
                            let pending = queue.get_pending_count(&job_id).await.unwrap_or(0);
                            let processing = queue.get_processing_count(&job_id).await.unwrap_or(0);
                            
                            if pending == 0 && processing == 0 {
                                // All tasks are done, update job status
                                if let Ok(mut status) = raw_storage.get_job_status(&job_id).await {
                                    if status.state != "completed" {
                                        status.state = "completed".to_string();
                                        status.updated_at = Utc::now();
                                        if let Err(e) = raw_storage.store_job_status(&status).await {
                                            error!("Failed to update job status: {}", e);
                                        }
                                    }
                                    
                                    info!("Worker {} completed job: {}", i, job_id);
                                    break;
                                }
                            }
                            
                            // Wait before checking again
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        },
                        Err(e) => {
                            error!("Worker {} failed to get task: {}", i, e);
                            
                            // Wait before retrying
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        }
                    }
                }
            });
        }
        
        Ok(())
    }
}