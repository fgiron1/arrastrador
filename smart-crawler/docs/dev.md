# Smart Crawler - Developer Documentation

## Overview

Smart Crawler is a modern, sophisticated web crawler designed with a focus on undetectability, scalability, and flexibility. The architecture follows a modular, distributed approach with a clear separation of responsibilities:

- **Rust Application**: Core orchestration, job management, configuration, and data storage
- **Python Browser Service**: Specialized browser automation, human-like behavior simulation, and website interaction
- **External Services**: Data persistence and task queue management (MongoDB, PostgreSQL, Redis)

This separation allows the crawler to leverage the best aspects of each technology:
- Rust provides performance, concurrency, and type safety for the core orchestration
- Python offers a rich ecosystem for browser automation (Selenium) and flexibility for custom site-specific scripts
- External services handle persistence, enabling distributed operation and scalability

## Architecture

### Rust Application Core

The Rust application is divided into several key modules with specific responsibilities:

#### CLI Module
- **Commands**: Parses and processes user commands (crawl, status, export, etc.)
- **Config**: Handles YAML-based configuration for crawler settings
- **Scripts**: Manages domain-specific automation scripts for the Python service

#### Browser Module
- **Fingerprint**: Generates realistic browser fingerprints to avoid detection
- **Remote**: Manages communication with the Python browser service
- **Script**: Handles uploading and managing custom Python scripts

#### Crawler Module
- **Controller**: Coordinates the crawling process, job management
- **Scheduler**: Determines which URLs should be crawled based on patterns and settings
- **Task**: Defines data structures for crawl tasks and results

#### Storage Module
- **Queue**: Manages the Redis-based task queue for crawler jobs
- **Raw**: Handles the storage of raw page data in MongoDB
- **Processed**: Manages processed data storage in PostgreSQL and export capabilities

#### Proxy Module
- **Manager**: Handles proxy configuration, testing, and rotation
- **VPN**: Manages VPN connections for IP rotation (optional)

#### Utils Module
- **Logging**: Configures tracing and logging for the application
- **Metrics**: Collects performance metrics during crawling

### Python Browser Service

The Python service is responsible for actual browser automation and implements:

1. **Flask API**: Provides endpoints for:
   - `/crawl`: Main endpoint for browser automation
   - `/health`: Health check endpoint
   - `/script/<domain>`: Script management endpoint

2. **Browser Utils**: A utility class with methods for:
   - Browser configuration with anti-detection measures
   - Human-like interaction (scrolling, clicking, typing)
   - Content extraction
   - Navigation assistance

3. **Custom Scripts System**: Allows for domain-specific crawling logic:
   - Supports per-domain customized automation
   - Scripts can be uploaded through the Rust CLI
   - Each script implements a `crawl()` function with domain-specific behavior

## Key Components in Detail

### Browser Fingerprinting

The system generates realistic browser fingerprints to avoid detection:

```rust
// In fingerprint.rs
pub struct CompleteFingerprint {
    pub name: String,
    pub user_agent: String,
    pub accept_language: String,
    pub platform: String,
    pub viewport: Viewport,
    pub headers: HashMap<String, String>,
    pub time_zone: Option<String>,
    pub webgl_vendor: Option<String>,
    pub webgl_renderer: Option<String>,
    pub has_touch: bool,
    pub color_depth: u32,
    pub hardware_concurrency: u32,
}
```

The Python service applies these fingerprints to the browser:

```python
# In browser_service.py
def configure_driver(browser_type, fingerprint):
    # Apply fingerprint settings to the browser
    options.add_argument(f"user-agent={fingerprint['user_agent']}")
    # ... other fingerprint settings
    
    # Anti-detection measures
    driver.execute_script("""
        Object.defineProperty(navigator, 'webdriver', {
            get: () => undefined
        });
    """)
```

### Human-like Behavior

The Python service simulates realistic human behavior:

```python
class BrowserUtils:
    @staticmethod
    def scroll(driver, amount=None, behavior='smooth', direction='down'):
        # Human-like scrolling behavior
        
    @staticmethod
    def human_click(driver, element):
        # Click with hover and realistic timing
        
    @staticmethod
    def human_type(driver, element, text, min_delay=0.05, max_delay=0.25):
        # Type with human-like delays between keystrokes
```

### Custom Domain Scripts

The system supports custom scripts for specific websites:

```python
# Example Wikipedia-specific script
def crawl(driver, url, behavior, utils):
    # Navigate to the URL
    driver.get(url)
    
    # Extract Wikipedia-specific data like:
    # - Article summary
    # - Infobox data
    # - Categories
    
    # Simulate human reading behavior
    if behavior.get("scroll_behavior") in ["random", "smooth"]:
        # Scroll down gradually as if reading
        
    # Sometimes interact with references or search
    
    return {
        "title": title,
        "content": content,
        "links": links,
        "article_data": article_data
    }
```

These scripts can be managed through the Rust CLI:

```rust
// In scripts.rs
pub async fn upload_script(domain: String, script_path: PathBuf) -> Result<()> {
    let browser_service = RemoteBrowserService::new();
    let script_manager = browser_service.script_manager();
    
    script_manager.upload_script(&domain, &script_path).await?;
    
    info!("Script uploaded successfully for domain: {}", domain);
    
    Ok(())
}
```

### Distributed Task Management

The system uses Redis for task distribution:

```rust
// In queue.rs
pub async fn push_task(&self, task: &CrawlTask) -> Result<()> {
    // Serialize task to JSON
    // Check if task is already being processed
    // Add task to the Redis queue
}

pub async fn pop_task(&self, job_id: &str) -> Result<Option<CrawlTask>> {
    // Get task from Redis queue
    // Add URL to processing set
    // Return task for processing
}
```

### Data Storage

The system uses multiple storage systems:

1. **MongoDB** for raw crawled data:
```rust
// In raw.rs
async fn store_page_result(&self, result: &TaskResult) -> Result<()> {
    // Convert to BSON document
    // Upsert into MongoDB collection
}
```

2. **PostgreSQL** for processed data:
```rust
// In processed.rs
async fn store_page_data(&self, job_id: &str, url: &str, data: serde_json::Value) -> Result<()> {
    // Insert/update in PostgreSQL
}
```

3. **Export capabilities** for extracted data:
```rust
async fn export_as_json(&self, job_id: &str, output_path: &Path) -> Result<()> {
    // Query data from database
    // Write JSON to file
}
```

## Workflow

1. **Job Initialization**:
   - User starts a job with `crawler crawl https://example.com`
   - Controller creates a job ID and initial task
   - Task is added to Redis queue

2. **Task Processing**:
   - Controller pops tasks from the queue
   - For each task, it communicates with the Python browser service
   - Browser service applies fingerprinting and human-like behavior
   - If a domain-specific script exists, it's used for custom behavior

3. **Data Collection**:
   - Browser service returns page content and discovered links
   - Controller normalizes and filters links based on patterns
   - New links are added to the queue for processing
   - Page data is stored in MongoDB (raw) and PostgreSQL (processed)

4. **Results Retrieval**:
   - User can check job status with `crawler status <job-id>`
   - Data can be exported with `crawler export <job-id> --format json`

## Deployment Options

### Standalone Mode

For simple use cases, you can run everything on a single machine:

```
crawler crawl https://example.com
```

### Kubernetes Deployment

For distributed crawling, the system supports Kubernetes:

1. **Controller Deployment**: Manages jobs and tasks
2. **Worker Deployment**: Processes crawl tasks
3. **Browser Service Deployment**: Runs the Python service
4. **Storage Services**: Redis, MongoDB, PostgreSQL

Configuration provided in `kubernetes/` directory includes:
- `controller.yaml`: Main controller deployment
- `worker.yaml`: Horizontal scaling workers
- `storage.yaml`: Database services
- `rbac.yaml`: Permissions for Kubernetes integration

## Docker Support

Docker configurations facilitate container deployment:

- `browser-service/Dockerfile`: Python browser service
- `build/build-browser-service.sh`: Build script for browser service
- `build/build-crawler.sh`: Build script for Rust application

## Extending the Crawler

### Adding a New Domain Script

1. Create a Python script for the domain:
```python
# domain_com.py
def crawl(driver, url, behavior, utils):
    # Domain-specific logic here
    return {
        "title": title,
        "content": content,
        "links": links,
        "custom_data": extracted_data
    }
```

2. Upload via CLI:
```bash
crawler script upload domain.com ./domain_com.py
```

### Adding New CLI Commands

Extend the `Commands` enum in `cli/mod.rs`:

```rust
enum Commands {
    // Existing commands...
    
    /// New custom command
    NewCommand {
        /// Parameter description
        #[arg(required = true)]
        parameter: String,
    },
}
```

Implement the command handler in `cli/commands.rs`:

```rust
pub async fn new_command(parameter: String) -> Result<()> {
    // Command implementation
    Ok(())
}
```

Add handling in `process_command`:

```rust
pub async fn process_command(cli: Cli) -> Result<()> {
    match cli.command {
        // Existing commands...
        
        Commands::NewCommand { parameter } => {
            commands::new_command(parameter).await
        },
    }
}
```

### Implementing Custom Behavior Patterns

To add new human-like behavior patterns, modify `BrowserUtils` in the Python service:

```python
@staticmethod
def new_behavior(driver, params):
    # Implement new behavior pattern
    pass
```

Then use it in domain-specific scripts or the default behavior.

## Troubleshooting

### Common Issues

1. **Redis Connection Problems**:
   - Check Redis connection string in configuration
   - Ensure Redis service is running

2. **MongoDB/PostgreSQL Issues**:
   - Verify connection strings and credentials
   - Check that databases and schemas exist

3. **Python Browser Service**:
   - Ensure the service is running and accessible
   - Check for WebDriver issues (ChromeDriver/GeckoDriver)

4. **Kubernetes Deployment**:
   - Verify pod status with `kubectl get pods`
   - Check logs with `kubectl logs <pod-name>`

### Logging and Debugging

- The Rust application uses `tracing` for structured logging
- Logs can be directed to console or file via configuration
- The Python service uses Python's logging module

To enable verbose logging in the Rust application:

```bash
RUST_LOG=debug crawler ...
```

## Performance Considerations

- The browser service is the main bottleneck for scaling
- Use multiple browser service instances for high-volume crawling
- Consider proxy rotation for large-scale operations
- MongoDB should be configured with appropriate indexing for large datasets
- Redis persistence should be enabled for reliability

## Security Notes

- Credentials are stored in configuration files
- Consider using Kubernetes secrets for production
- Proxy and VPN information should be protected
- The browser service should be secured behind a firewall

## Conclusion

Smart Crawler is designed for flexibility, scalability, and undetectability. The modular architecture allows for easy customization and extension, while the separation between Rust and Python components leverages the strengths of both languages.

The system's ability to use domain-specific scripts makes it adaptable to a wide range of websites and use cases, from simple data collection to complex interactions requiring human-like behavior.