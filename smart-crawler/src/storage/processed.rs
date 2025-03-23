use anyhow::{Result, Context};
use async_trait::async_trait;
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use sqlx::types::Json;
use serde::{Serialize, Deserialize};
use serde_json; // Add this import
use std::path::Path;
use std::sync::Arc;
use std::fs;
use std::io::Write;
use tracing::{debug, error};
use chrono::{DateTime, Utc};

use crate::cli::config::ProcessedDataSettings;
use crate::crawler::task::TaskResult;

/// Trait for processed data storage
#[async_trait]
pub trait ProcessedStorage: Send + Sync {
    /// Store processed page data
    async fn store_page_data(&self, job_id: &str, url: &str, data: serde_json::Value) -> Result<()>;
    
    /// Get processed page data
    async fn get_page_data(&self, job_id: &str, url: &str) -> Result<Option<serde_json::Value>>;
    
    /// List all pages for a job
    async fn list_pages(&self, job_id: &str) -> Result<Vec<String>>;
    
    /// Export job data as JSON
    async fn export_as_json(&self, job_id: &str, output_path: &Path) -> Result<()>;
    
    /// Export job data as CSV
    async fn export_as_csv(&self, job_id: &str, output_path: &Path) -> Result<()>;
    
    /// Export job data as SQL
    async fn export_as_sql(&self, job_id: &str, output_path: &Path) -> Result<()>;
    
    /// Delete a job and all its data
    async fn delete_job(&self, job_id: &str) -> Result<()>;
}

/// Factory for creating a ProcessedStorage implementation
pub struct ProcessedStorageFactory;

impl ProcessedStorageFactory {
    /// Create a new ProcessedStorage instance based on the settings
    pub async fn create(settings: &ProcessedDataSettings) -> Result<Arc<dyn ProcessedStorage>> {
        match settings.storage_type.as_str() {
            "postgresql" => {
                let storage = PostgresStorage::new(settings).await?;
                Ok(Arc::new(storage))
            },
            "sqlite" => {
                // For future implementation
                anyhow::bail!("SQLite storage is not yet implemented");
            },
            "filesystem" => {
                // For future implementation
                anyhow::bail!("Filesystem storage is not yet implemented");
            },
            _ => {
                anyhow::bail!("Unsupported processed data storage type: {}", settings.storage_type);
            }
        }
    }
    
    /// Connect to an existing ProcessedStorage instance
    pub async fn connect(settings: &ProcessedDataSettings) -> Result<Arc<dyn ProcessedStorage>> {
        Self::create(settings).await
    }
}

/// PostgreSQL implementation of ProcessedStorage
pub struct PostgresStorage {
    /// PostgreSQL connection pool
    pool: Pool<Postgres>,
    
    /// Schema name
    schema: String,
    
    /// Table prefix
    table_prefix: String,
}

/// Page data record for database storage
#[derive(Debug, Serialize, Deserialize)]
struct PageData {
    job_id: String,
    url: String,
    data: Json<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}




impl PostgresStorage {
    /// Create a new PostgreSQL storage instance
    pub async fn new(settings: &ProcessedDataSettings) -> Result<Self> {
        // Create connection pool
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&settings.connection_string)
            .await
            .context(format!("Failed to connect to PostgreSQL: {}", settings.connection_string))?;
        
        let storage = Self {
            pool,
            schema: settings.schema_name.clone(),
            table_prefix: settings.table_prefix.clone(),
        };
        
        // Ensure schema exists
        storage.ensure_schema().await?;
        
        debug!("Connected to PostgreSQL database");
        
        Ok(storage)
    }
    
    /// Ensure the schema exists
    async fn ensure_schema(&self) -> Result<()> {
        let query = format!("CREATE SCHEMA IF NOT EXISTS {}", self.schema);
        
        sqlx::query(&query)
            .execute(&self.pool)
            .await
            .context(format!("Failed to create schema: {}", self.schema))?;
        
        debug!("Ensured schema exists: {}", self.schema);
        
        Ok(())
    }
    
    /// Ensure the pages table exists for a job
    async fn ensure_pages_table(&self, job_id: &str) -> Result<()> {
        let table_name = self.get_pages_table_name(job_id);
        
        let query = format!(
            "CREATE TABLE IF NOT EXISTS {}.{} (
                job_id TEXT NOT NULL,
                url TEXT NOT NULL,
                data JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                PRIMARY KEY (job_id, url)
            )",
            self.schema, table_name
        );
        
        sqlx::query(&query)
            .execute(&self.pool)
            .await
            .context(format!("Failed to create pages table: {}", table_name))?;
        
        debug!("Ensured pages table exists: {}", table_name);
        
        Ok(())
    }
    
    /// Get the name of the pages table for a job
    fn get_pages_table_name(&self, job_id: &str) -> String {
        format!("{}_{}_pages", self.table_prefix, job_id.replace('-', "_"))
    }
}

#[async_trait]
impl ProcessedStorage for PostgresStorage {
    async fn store_page_data(&self, job_id: &str, url: &str, data: serde_json::Value) -> Result<()> {
        // Ensure the pages table exists
        self.ensure_pages_table(job_id).await?;
        
        let table_name = self.get_pages_table_name(job_id);
        
        // Insert or update the page data
        let query = format!(
            "INSERT INTO {}.{} (job_id, url, data, created_at, updated_at)
             VALUES ($1, $2, $3, NOW(), NOW())
             ON CONFLICT (job_id, url) DO UPDATE
             SET data = $3, updated_at = NOW()",
            self.schema, table_name
        );
        
        sqlx::query(&query)
            .bind(job_id)
            .bind(url)
            .bind(&Json(data))
            .execute(&self.pool)
            .await
            .context("Failed to store page data in PostgreSQL")?;
        
        debug!("Stored processed data for URL: {}", url);
        
        Ok(())
    }
    
    async fn get_page_data(&self, job_id: &str, url: &str) -> Result<Option<serde_json::Value>> {
        let table_name = self.get_pages_table_name(job_id);
        
        // Check if the table exists
        let table_exists = sqlx::query_scalar::<_, bool>(
            &format!(
                "SELECT EXISTS (
                    SELECT FROM pg_tables
                    WHERE schemaname = $1 AND tablename = $2
                )",
            )
        )
        .bind(&self.schema)
        .bind(&table_name)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check if table exists")?;
        
        if !table_exists {
            return Ok(None);
        }
        
        // Query the page data
        let query = format!(
            "SELECT data FROM {}.{} WHERE job_id = $1 AND url = $2",
            self.schema, table_name
        );
        
        let result: Option<Json<serde_json::Value>> = sqlx::query_scalar(&query)
            .bind(job_id)
            .bind(url)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to query page data from PostgreSQL")?;
        
        // Convert to serde_json::Value if found
        Ok(result.map(|json| json.0))
    }
    
    async fn list_pages(&self, job_id: &str) -> Result<Vec<String>> {
        let table_name = self.get_pages_table_name(job_id);
        
        // Check if the table exists
        let table_exists = sqlx::query_scalar::<_, bool>(
            &format!(
                "SELECT EXISTS (
                    SELECT FROM pg_tables
                    WHERE schemaname = $1 AND tablename = $2
                )",
            )
        )
        .bind(&self.schema)
        .bind(&table_name)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check if table exists")?;
        
        if !table_exists {
            return Ok(Vec::new());
        }
        
        // Query all page URLs
        let query = format!(
            "SELECT url FROM {}.{} WHERE job_id = $1 ORDER BY url",
            self.schema, table_name
        );
        
        let results: Vec<String> = sqlx::query_scalar(&query)
            .bind(job_id)
            .fetch_all(&self.pool)
            .await
            .context("Failed to query page URLs from PostgreSQL")?;
        
        Ok(results)
    }
    
    async fn export_as_json(&self, job_id: &str, output_path: &Path) -> Result<()> {
        let table_name = self.get_pages_table_name(job_id);
        
        // Check if the table exists
        let table_exists = sqlx::query_scalar::<_, bool>(
            &format!(
                "SELECT EXISTS (
                    SELECT FROM pg_tables
                    WHERE schemaname = $1 AND tablename = $2
                )",
            )
        )
        .bind(&self.schema)
        .bind(&table_name)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check if table exists")?;
        
        if !table_exists {
            // Create an empty JSON array if no data
            let file = fs::File::create(output_path)
                .context(format!("Failed to create output file: {}", output_path.display()))?;
            
            serde_json::to_writer_pretty(file, &Vec::<serde_json::Value>::new())
                .context("Failed to write empty JSON array to file")?;
            
            return Ok(());
        }
        
        // Query all page data
        let query = format!(
            "SELECT json_build_object(
                'job_id', job_id,
                'url', url,
                'data', data,
                'created_at', created_at,
                'updated_at', updated_at
            ) AS json_data
            FROM {}.{}
            WHERE job_id = $1
            ORDER BY url",
            self.schema, table_name
        );
        
        let results: Vec<serde_json::Value> = sqlx::query_scalar(&query)
            .bind(job_id)
            .fetch_all(&self.pool)
            .await
            .context("Failed to query page data from PostgreSQL")?;
        
        // Write to file
        let file = fs::File::create(output_path)
            .context(format!("Failed to create output file: {}", output_path.display()))?;
        
        serde_json::to_writer_pretty(file, &results)
            .context("Failed to write JSON data to file")?;
        
        debug!("Exported {} records to JSON file: {}", results.len(), output_path.display());
        
        Ok(())
    }
    
    async fn export_as_csv(&self, job_id: &str, output_path: &Path) -> Result<()> {
        let table_name = self.get_pages_table_name(job_id);
        
        // Check if the table exists
        let table_exists = sqlx::query_scalar::<_, bool>(
            &format!(
                "SELECT EXISTS (
                    SELECT FROM pg_tables
                    WHERE schemaname = $1 AND tablename = $2
                )",
            )
        )
        .bind(&self.schema)
        .bind(&table_name)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check if table exists")?;
        
        if !table_exists {
            // Create an empty CSV file if no data
            let mut file = fs::File::create(output_path)
                .context(format!("Failed to create output file: {}", output_path.display()))?;
            
            // Write header row
            writeln!(file, "job_id,url,created_at,updated_at")
                .context("Failed to write CSV header to file")?;
            
            return Ok(());
        }
        
        // Query all page data
        let query = format!(
            "SELECT job_id, url, created_at, updated_at
            FROM {}.{}
            WHERE job_id = $1
            ORDER BY url",
            self.schema, table_name
        );
        
        #[derive(sqlx::FromRow)]
        struct CsvRow {
            job_id: String,
            url: String,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
        }
        
        let results = sqlx::query_as::<_, CsvRow>(&query)
            .bind(job_id)
            .fetch_all(&self.pool)
            .await
            .context("Failed to query page data from PostgreSQL")?;
        
        // Write to CSV file
        let mut file = fs::File::create(output_path)
            .context(format!("Failed to create output file: {}", output_path.display()))?;
        
        // Write header row
        writeln!(file, "job_id,url,created_at,updated_at")
            .context("Failed to write CSV header to file")?;
        let results_length = results.len(); 
        // Write data rows
        for row in results {
            writeln!(
                file,
                "{},{},{},{}",
                row.job_id,
                row.url,
                row.created_at.to_rfc3339(),
                row.updated_at.to_rfc3339()
            )
            .context("Failed to write CSV row to file")?;
        }
        
        debug!("Exported {} records to CSV file: {}", results_length, output_path.display());
        
        Ok(())
    }
    
    async fn export_as_sql(&self, job_id: &str, output_path: &Path) -> Result<()> {
        let table_name = self.get_pages_table_name(job_id);
        
        // Check if the table exists
        let table_exists = sqlx::query_scalar::<_, bool>(
            &format!(
                "SELECT EXISTS (
                    SELECT FROM pg_tables
                    WHERE schemaname = $1 AND tablename = $2
                )",
            )
        )
        .bind(&self.schema)
        .bind(&table_name)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check if table exists")?;
        
        if !table_exists {
            // Create an empty SQL file if no data
            let mut file = fs::File::create(output_path)
                .context(format!("Failed to create output file: {}", output_path.display()))?;
            
            // Write table creation statement
            write!(
                file,
                "CREATE TABLE IF NOT EXISTS crawled_data (
                    job_id TEXT NOT NULL,
                    url TEXT NOT NULL,
                    data JSONB NOT NULL,
                    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
                    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
                    PRIMARY KEY (job_id, url)
                );\n"
            )
            .context("Failed to write SQL create table statement to file")?;
            
            return Ok(());
        }
        
        // Query all page data
        let query = format!(
            "SELECT job_id, url, data, created_at, updated_at
            FROM {}.{}
            WHERE job_id = $1
            ORDER BY url",
            self.schema, table_name
        );
        
        #[derive(sqlx::FromRow)]
        struct SqlRow {
            job_id: String,
            url: String,
            data: Json<serde_json::Value>,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
        }

        let results = sqlx::query_as::<_, SqlRow>(&query)
            .bind(job_id)
            .fetch_all(&self.pool)
            .await
            .context("Failed to query page data from PostgreSQL")?;
        
        // Write to SQL file
        let mut file = fs::File::create(output_path)
            .context(format!("Failed to create output file: {}", output_path.display()))?;
        
        // Write table creation statement
        write!(
            file,
            "CREATE TABLE IF NOT EXISTS crawled_data (
                job_id TEXT NOT NULL,
                url TEXT NOT NULL,
                data JSONB NOT NULL,
                created_at TIMESTAMP WITH TIME ZONE NOT NULL,
                updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
                PRIMARY KEY (job_id, url)
            );\n\n"
        )
        .context("Failed to write SQL create table statement to file")?;
        let result_count = results.len();
        // Write data insert statements
        for row in results {
            let data_json = serde_json::to_string(&row.data.0)
                .context("Failed to serialize JSON data")?;
            
            writeln!(
                file,
                "INSERT INTO crawled_data (job_id, url, data, created_at, updated_at) VALUES ('{}', '{}', '{}', '{}', '{}');",
                row.job_id.replace('\'', "''"),
                row.url.replace('\'', "''"),
                data_json.replace('\'', "''"),
                row.created_at.to_rfc3339(),
                row.updated_at.to_rfc3339()
            )
            .context("Failed to write SQL insert statement to file")?;
        }
        
        debug!("Exported {} records to SQL file: {}", result_count, output_path.display());
        
        Ok(())
    }
    
    async fn delete_job(&self, job_id: &str) -> Result<()> {
        let table_name = self.get_pages_table_name(job_id);
        
        // Check if the table exists
        let table_exists = sqlx::query_scalar::<_, bool>(
            &format!(
                "SELECT EXISTS (
                    SELECT FROM pg_tables
                    WHERE schemaname = $1 AND tablename = $2
                )",
            )
        )
        .bind(&self.schema)
        .bind(&table_name)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check if table exists")?;
        
        if !table_exists {
            // Table doesn't exist, nothing to delete
            return Ok(());
        }
        
        // Drop the table
        let query = format!("DROP TABLE {}.{}", self.schema, table_name);
        
        sqlx::query(&query)
            .execute(&self.pool)
            .await
            .context(format!("Failed to drop table: {}", table_name))?;
        
        debug!("Deleted job data: {}", job_id);
        
        Ok(())
    }
}