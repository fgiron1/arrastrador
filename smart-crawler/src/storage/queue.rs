use anyhow::{Result, Context};
use redis::{Client, aio::MultiplexedConnection};
use tracing::{debug, error};
use tokio::sync::Mutex;
use std::sync::Arc;

use crate::cli::config::QueueSettings;
use crate::crawler::task::CrawlTask;

/// Queue manager for task distribution
pub struct QueueManager {
    /// Redis client
    client: Client,
    
    /// Task TTL in seconds
    task_ttl: u64,
    
    /// Connection pool
    conn_pool: Arc<Mutex<MultiplexedConnection>>,
}

impl QueueManager {
    /// Create a new queue manager
    pub async fn new(config: &QueueSettings) -> Result<Self> {
        let client = Client::open(config.redis_url.clone())
            .context(format!("Failed to connect to Redis at {}", config.redis_url))?;
        
        let conn = client.get_multiplexed_async_connection().await
            .context("Failed to get Redis connection")?;
        
        let conn_pool = Arc::new(Mutex::new(conn));
        
        Ok(Self {
            client,
            task_ttl: config.task_ttl,
            conn_pool,
        })
    }
    
    /// Connect to an existing queue
    pub async fn connect(config: &QueueSettings) -> Result<Self> {
        Self::new(config).await
    }
    
    /// Push a task to the queue
    pub async fn push_task(&self, task: &CrawlTask) -> Result<()> {
        let task_json = serde_json::to_string(task)
            .context("Failed to serialize task")?;
        
        let queue_key = format!("crawler:queue:{}", task.job_id);
        let processing_key = format!("crawler:processing:{}", task.job_id);
        
        let mut conn = self.conn_pool.lock().await;
        
        // Check if the task is already in processing
        let in_processing: bool = redis::cmd("SISMEMBER")
            .arg(&processing_key)
            .arg(&task.url)
            .query_async(&mut *conn)
            .await
            .unwrap_or(false);
        
        if in_processing {
            debug!("Skipping task for URL that's already processing: {}", task.url);
            return Ok(());
        }
        
        // Add task to the queue
        redis::cmd("LPUSH")
            .arg(&queue_key)
            .arg(&task_json)
            .query_async::<_, ()>(&mut *conn)
            .await
            .context("Failed to push task to Redis queue")?;
        
        // Set TTL on the queue if not already set
        let ttl: i64 = redis::cmd("TTL")
            .arg(&queue_key)
            .query_async(&mut *conn)
            .await
            .unwrap_or(-1);
        
        if ttl == -1 {
            redis::cmd("EXPIRE")
                .arg(&queue_key)
                .arg(self.task_ttl)
                .query_async::<_, ()>(&mut *conn)
                .await
                .context("Failed to set TTL on queue")?;
        }
        
        debug!("Pushed task to queue: {}", task.url);
        
        Ok(())
    }
    
    /// Pop a task from the queue
    pub async fn pop_task(&self, job_id: &str) -> Result<Option<CrawlTask>> {
        let queue_key = format!("crawler:queue:{}", job_id);
        let processing_key = format!("crawler:processing:{}", job_id);
        
        let mut conn = self.conn_pool.lock().await;
        
        // Get a task from the queue
        let task_json: Option<String> = redis::cmd("RPOP")
            .arg(&queue_key)
            .query_async(&mut *conn)
            .await
            .context("Failed to pop task from Redis queue")?;
        
        if let Some(task_json) = task_json {
            // Parse the task
            let task: CrawlTask = serde_json::from_str(&task_json)
                .context("Failed to deserialize task")?;
            
            // Add the URL to the processing set
            redis::cmd("SADD")
                .arg(&processing_key)
                .arg(&task.url)
                .query_async::<_, ()>(&mut *conn)
                .await
                .context("Failed to add URL to processing set")?;
            
            // Set TTL on the processing set if not already set
            let ttl: i64 = redis::cmd("TTL")
                .arg(&processing_key)
                .query_async(&mut *conn)
                .await
                .unwrap_or(-1);
            
            if ttl == -1 {
                redis::cmd("EXPIRE")
                    .arg(&processing_key)
                    .arg(self.task_ttl)
                    .query_async::<_, ()>(&mut *conn)
                    .await
                    .context("Failed to set TTL on processing set")?;
            }
            
            debug!("Popped task from queue: {}", task.url);
            
            Ok(Some(task))
        } else {
            Ok(None)
        }
    }
    
    /// Mark a task as completed
    pub async fn complete_task(&self, job_id: &str, url: &str) -> Result<()> {
        let processing_key = format!("crawler:processing:{}", job_id);
        let completed_key = format!("crawler:completed:{}", job_id);
        
        let mut conn = self.conn_pool.lock().await;
        
        // Remove the URL from the processing set
        redis::cmd("SREM")
            .arg(&processing_key)
            .arg(url)
            .query_async::<_, ()>(&mut *conn)
            .await
            .context("Failed to remove URL from processing set")?;
        
        // Add the URL to the completed set
        redis::cmd("SADD")
            .arg(&completed_key)
            .arg(url)
            .query_async::<_, ()>(&mut *conn)
            .await
            .context("Failed to add URL to completed set")?;
        
        // Set TTL on the completed set if not already set
        let ttl: i64 = redis::cmd("TTL")
            .arg(&completed_key)
            .query_async(&mut *conn)
            .await
            .unwrap_or(-1);
        
        if ttl == -1 {
            redis::cmd("EXPIRE")
                .arg(&completed_key)
                .arg(self.task_ttl)
                .query_async::<_, ()>(&mut *conn)
                .await
                .context("Failed to set TTL on completed set")?;
        }
        
        debug!("Marked task as completed: {}", url);
        
        Ok(())
    }
    
    /// Mark a task as failed
    pub async fn fail_task(&self, job_id: &str, url: &str, error: &str) -> Result<()> {
        let processing_key = format!("crawler:processing:{}", job_id);
        let failed_key = format!("crawler:failed:{}", job_id);
        let error_key = format!("crawler:errors:{}:{}", job_id, url);
        
        let mut conn = self.conn_pool.lock().await;
        
        // Remove the URL from the processing set
        redis::cmd("SREM")
            .arg(&processing_key)
            .arg(url)
            .query_async::<_, ()>(&mut *conn)
            .await
            .context("Failed to remove URL from processing set")?;
        
        // Add the URL to the failed set
        redis::cmd("SADD")
            .arg(&failed_key)
            .arg(url)
            .query_async::<_, ()>(&mut *conn)
            .await
            .context("Failed to add URL to failed set")?;
        
        // Store the error message
        redis::cmd("SET")
            .arg(&error_key)
            .arg(error)
            .query_async::<_, ()>(&mut *conn)
            .await
            .context("Failed to store error message")?;
        
        // Set TTLs
        let ttl: i64 = redis::cmd("TTL")
            .arg(&failed_key)
            .query_async(&mut *conn)
            .await
            .unwrap_or(-1);
        
        if ttl == -1 {
            redis::cmd("EXPIRE")
                .arg(&failed_key)
                .arg(self.task_ttl)
                .query_async::<_, ()>(&mut *conn)
                .await
                .context("Failed to set TTL on failed set")?;
            
            redis::cmd("EXPIRE")
                .arg(&error_key)
                .arg(self.task_ttl)
                .query_async::<_, ()>(&mut *conn)
                .await
                .context("Failed to set TTL on error message")?;
        }
        
        debug!("Marked task as failed: {}", url);
        
        Ok(())
    }
    
    /// Get the number of pending tasks for a job
    pub async fn get_pending_count(&self, job_id: &str) -> Result<usize> {
        let queue_key = format!("crawler:queue:{}", job_id);
        
        let mut conn = self.conn_pool.lock().await;
        
        let count: usize = redis::cmd("LLEN")
            .arg(&queue_key)
            .query_async(&mut *conn)
            .await
            .context("Failed to get queue length")?;
        
        Ok(count)
    }
    
    /// Get the number of processing tasks for a job
    pub async fn get_processing_count(&self, job_id: &str) -> Result<usize> {
        let processing_key = format!("crawler:processing:{}", job_id);
        
        let mut conn = self.conn_pool.lock().await;
        
        let count: usize = redis::cmd("SCARD")
            .arg(&processing_key)
            .query_async(&mut *conn)
            .await
            .context("Failed to get processing set size")?;
        
        Ok(count)
    }
    
    /// Get the number of completed tasks for a job
    pub async fn get_completed_count(&self, job_id: &str) -> Result<usize> {
        let completed_key = format!("crawler:completed:{}", job_id);
        
        let mut conn = self.conn_pool.lock().await;
        
        let count: usize = redis::cmd("SCARD")
            .arg(&completed_key)
            .query_async(&mut *conn)
            .await
            .context("Failed to get completed set size")?;
        
        Ok(count)
    }
    
    /// Get the number of failed tasks for a job
    pub async fn get_failed_count(&self, job_id: &str) -> Result<usize> {
        let failed_key = format!("crawler:failed:{}", job_id);
        
        let mut conn = self.conn_pool.lock().await;
        
        let count: usize = redis::cmd("SCARD")
            .arg(&failed_key)
            .query_async(&mut *conn)
            .await
            .context("Failed to get failed set size")?;
        
        Ok(count)
    }
    
    /// Clear all data for a job
    pub async fn clear_job(&self, job_id: &str) -> Result<()> {
        let queue_key = format!("crawler:queue:{}", job_id);
        let processing_key = format!("crawler:processing:{}", job_id);
        let completed_key = format!("crawler:completed:{}", job_id);
        let failed_key = format!("crawler:failed:{}", job_id);
        let error_pattern = format!("crawler:errors:{}:*", job_id);
        
        let mut conn = self.conn_pool.lock().await;
        
        // Delete the queue
        redis::cmd("DEL")
            .arg(&queue_key)
            .query_async::<_, ()>(&mut *conn)
            .await
            .context("Failed to delete queue")?;
        
        // Delete the sets
        redis::cmd("DEL")
            .arg(&processing_key)
            .arg(&completed_key)
            .arg(&failed_key)
            .query_async::<_, ()>(&mut *conn)
            .await
            .context("Failed to delete sets")?;
        
        // Find and delete all error messages
        let error_keys: Vec<String> = redis::cmd("KEYS")
            .arg(&error_pattern)
            .query_async(&mut *conn)
            .await
            .context("Failed to get error keys")?;
        
        if !error_keys.is_empty() {
            redis::cmd("DEL")
                .arg(&error_keys)
                .query_async::<_, ()>(&mut *conn)
                .await
                .context("Failed to delete error messages")?;
        }
        
        debug!("Cleared all data for job: {}", job_id);
        
        Ok(())
    }
}