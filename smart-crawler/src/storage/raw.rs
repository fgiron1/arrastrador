use anyhow::{Result, Context};
use futures::StreamExt;
use async_trait::async_trait;
use mongodb::{Client, Database, Collection, options::ClientOptions};
use mongodb::bson::{doc, Document};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tracing::debug;
use chrono::{DateTime, Utc}; // Make sure to add this

use crate::cli::config::RawDataSettings;
use crate::crawler::task::TaskResult;

// Define the JobStatus struct here to avoid circular dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatus {
    pub job_id: String,
    pub seed_url: String,
    pub state: String,  // "pending", "running", "completed", "failed"
    pub pages_crawled: usize,
    pub pages_total: usize,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub errors: Vec<String>,
}

/// Trait for raw data storage
#[async_trait]
pub trait RawStorageBackend: Send + Sync {
    /// Store a page result
    async fn store_page_result(&self, result: &TaskResult) -> Result<()>;
    
    /// Get a page result by URL
    async fn get_page_result(&self, job_id: &str, url: &str) -> Result<Option<TaskResult>>;
    
    /// Store job status
    async fn store_job_status(&self, status: &JobStatus) -> Result<()>;
    
    /// Get job status
    async fn get_job_status(&self, job_id: &str) -> Result<JobStatus>;
    
    /// List all jobs
    async fn list_jobs(&self) -> Result<Vec<JobStatus>>;
    
    /// Delete a job and all its data
    async fn delete_job(&self, job_id: &str) -> Result<()>;
}

/// Factory for creating a RawStorage implementation
pub struct RawStorage;

impl RawStorage {
    /// Create a new RawStorage instance based on the settings
    pub async fn create(settings: &RawDataSettings) -> Result<Arc<dyn RawStorageBackend>> {
        match settings.storage_type.as_str() {
            "mongodb" => {
                let storage = MongoDBStorage::new(settings).await?;
                Ok(Arc::new(storage))
            },
            "filesystem" => {
                // For future implementation
                anyhow::bail!("Filesystem storage is not yet implemented");
            },
            _ => {
                anyhow::bail!("Unsupported raw data storage type: {}", settings.storage_type);
            }
        }
    }
    
    /// Connect to an existing RawStorage instance
    pub async fn connect(settings: &RawDataSettings) -> Result<Arc<dyn RawStorageBackend>> {
        Self::create(settings).await
    }
}

/// MongoDB implementation of RawStorage
pub struct MongoDBStorage {
    /// MongoDB client
    client: Client,
    
    /// MongoDB database
    database: Database,
    
    /// Collection prefix
    collection_prefix: String,
}

impl MongoDBStorage {
    /// Create a new MongoDB storage instance
    pub async fn new(settings: &RawDataSettings) -> Result<Self> {
        // Parse connection options
        let client_options = ClientOptions::parse(&settings.connection_string)
            .await
            .context(format!("Failed to parse MongoDB connection string: {}", settings.connection_string))?;
        
        // Create the client
        let client = Client::with_options(client_options)
            .context("Failed to create MongoDB client")?;
        
        // Get the database
        let database = client.database(&settings.database_name);
        
        // Test connection
        database.list_collection_names(None)
            .await
            .context("Failed to connect to MongoDB")?;
        
        debug!("Connected to MongoDB database: {}", settings.database_name);
        
        Ok(Self {
            client,
            database,
            collection_prefix: settings.collection_prefix.clone(),
        })
    }
    
    /// Get the collection for page results
    fn pages_collection(&self, job_id: &str) -> Collection<Document> {
        self.database.collection(&format!("{}_{}_pages", self.collection_prefix, job_id))
    }
    
    /// Get the collection for job status
    fn jobs_collection(&self) -> Collection<Document> {
        self.database.collection(&format!("{}_jobs", self.collection_prefix))
    }
}

#[async_trait]
impl RawStorageBackend for MongoDBStorage {
    async fn store_page_result(&self, result: &TaskResult) -> Result<()> {
        let collection = self.pages_collection(&result.job_id);
        
        // Convert to BSON document
        let doc = mongodb::bson::to_document(result)
            .context("Failed to convert TaskResult to BSON document")?;
        
        // Create filter for upsert
        let filter = doc! {
            "job_id": &result.job_id,
            "url": &result.url,
        };
        
        // Upsert the document
        collection.replace_one(filter, doc, mongodb::options::ReplaceOptions::builder().upsert(true).build())
            .await
            .context("Failed to store page result in MongoDB")?;
        
        debug!("Stored page result for URL: {}", result.url);
        
        Ok(())
    }
    
    async fn get_page_result(&self, job_id: &str, url: &str) -> Result<Option<TaskResult>> {
        let collection = self.pages_collection(job_id);
        
        // Create filter
        let filter = doc! {
            "job_id": job_id,
            "url": url,
        };
        
        // Find the document
        let result = collection.find_one(filter, None).await
            .context("Failed to query MongoDB for page result")?;
        
        // Convert to TaskResult if found
        if let Some(doc) = result {
            let task_result: TaskResult = mongodb::bson::from_document(doc)
                .context("Failed to convert BSON document to TaskResult")?;
            
            Ok(Some(task_result))
        } else {
            Ok(None)
        }
    }
    
    async fn store_job_status(&self, status: &JobStatus) -> Result<()> {
        let collection = self.jobs_collection();
        
        // Convert to BSON document
        let doc = mongodb::bson::to_document(status)
            .context("Failed to convert JobStatus to BSON document")?;
        
        // Create filter for upsert
        let filter = doc! {
            "job_id": &status.job_id,
        };
        
        // Upsert the document
        collection.replace_one(filter, doc, mongodb::options::ReplaceOptions::builder().upsert(true).build())
            .await
            .context("Failed to store job status in MongoDB")?;
        
        debug!("Stored status for job: {}", status.job_id);
        
        Ok(())
    }
    
    async fn get_job_status(&self, job_id: &str) -> Result<JobStatus> {
        let collection = self.jobs_collection();
        
        // Create filter
        let filter = doc! {
            "job_id": job_id,
        };
        
        // Find the document
        let result = collection.find_one(filter, None).await
            .context("Failed to query MongoDB for job status")?;
        
        // Convert to JobStatus if found
        if let Some(doc) = result {
            let job_status: JobStatus = mongodb::bson::from_document(doc)
                .context("Failed to convert BSON document to JobStatus")?;
            
            Ok(job_status)
        } else {
            anyhow::bail!("Job not found: {}", job_id)
        }
    }
    
    async fn list_jobs(&self) -> Result<Vec<JobStatus>> {
        let collection = self.jobs_collection();
        
        // Find all job documents
        let mut cursor = collection.find(None, None).await
            .context("Failed to query MongoDB for jobs")?;
        
        // Convert to JobStatus objects
        let mut results = Vec::new();
        while let Some(doc) = cursor.next().await {
            let doc = doc.context("Failed to get document from cursor")?;
            results.push(doc);
        }
        let mut jobs = Vec::new();
        for doc in results {
            let job_status: JobStatus = mongodb::bson::from_document(doc)
                .context("Failed to convert BSON document to JobStatus")?;
            
            jobs.push(job_status);
        }
        
        Ok(jobs)
    }
    
    async fn delete_job(&self, job_id: &str) -> Result<()> {
        // Delete job status
        let jobs_collection = self.jobs_collection();
        let filter = doc! {
            "job_id": job_id,
        };
        
        jobs_collection.delete_one(filter, None).await
            .context("Failed to delete job status from MongoDB")?;
        
        // Delete page results
        let pages_collection = self.pages_collection(job_id);
        pages_collection.drop(None).await
            .context("Failed to drop pages collection from MongoDB")?;
        
        debug!("Deleted job and all its data: {}", job_id);
        
        Ok(())
    }
}