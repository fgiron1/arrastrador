use crate::browser::remote::RemoteBrowserService;

pub struct CrawlerController {
    config: CrawlerConfig,
    queue: Arc<QueueManager>,
    scheduler: Arc<Mutex<Scheduler>>,
    raw_storage: Arc<dyn RawStorage>,
    processed_storage: Arc<dyn ProcessedStorage>,
    browser_service: Arc<RemoteBrowserService>,  // Add this line
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
        
        // Initialize browser service - add this
        let browser_service = Arc::new(RemoteBrowserService::new("http://localhost:5000"));
        
        Ok(Self {
            config,
            queue,
            scheduler,
            raw_storage,
            processed_storage,
            browser_service,  // Add this
        })
    }
    
    // Also modify the connect method
    pub async fn connect() -> Result<Self> {
        // Load the default configuration
        let config = CrawlerConfig::load_default()?;
        
        // Connect to existing components rather than creating new ones
        let queue = Arc::new(QueueManager::connect(&config.storage.queue).await?);
        let raw_storage = RawStorage::connect(&config.storage.raw_data).await?;
        let processed_storage = ProcessedStorage::connect(&config.storage.processed_data).await?;
        
        // Create a new scheduler (stateless component)
        let scheduler = Arc::new(Mutex::new(Scheduler::new(config.crawler.clone())));
        
        // Initialize browser service - add this
        let browser_service = Arc::new(RemoteBrowserService::new("http://localhost:5000"));
        
        Ok(Self {
            config,
            queue,
            scheduler,
            raw_storage,
            processed_storage,
            browser_service,
        })
    }
    
    // Modify the process_task method
    async fn process_task(
        task: CrawlTask,
        config: &CrawlerConfig,
        scheduler: Arc<Mutex<Scheduler>>,
        raw_storage: Arc<dyn RawStorage>,
        queue: Arc<QueueManager>,
        browser_service: Arc<RemoteBrowserService>,  // Add this parameter
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
    
    // Update the start_workers method if you're using standalone mode
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
            let browser_service = self.browser_service.clone(); // Add this
            
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
                                browser_service.clone(), // Add this
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
                        // Rest of the code remains unchanged
                        // ...
                    }
                }
            });
        }
        
        Ok(())
    }
}