use anyhow::Result;
use tracing_subscriber::{fmt, EnvFilter};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use std::path::PathBuf;
use std::fs;

/// Initialize the logging system
pub fn init_logging(verbose: bool, log_file: Option<PathBuf>) -> Result<()> {
    // Create an environment filter
    let env_filter = if verbose {
        EnvFilter::from_default_env()
            .add_directive("smart_crawler=debug".parse()?)
            .add_directive("warn".parse()?)
    } else {
        EnvFilter::from_default_env()
            .add_directive("smart_crawler=info".parse()?)
            .add_directive("warn".parse()?)
    };
    
    // Configure the logging format
    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_span_events(FmtSpan::CLOSE);
    
    // If a log file is specified, create a file logger as well
    if let Some(log_file) = log_file {
        // Create parent directory if necessary
        if let Some(parent) = log_file.parent() {
            fs::create_dir_all(parent)?;
        }
        
        // Create the file writer
        let file = fs::File::create(log_file)?;
        let file_layer = fmt::layer()
            .with_target(true)
            .with_ansi(false)
            .with_writer(file);
        
        // Initialize the registry with both loggers
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(file_layer)
            .init();
    } else {
        // Just use the standard logger
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    }
    
    Ok(())
}

/// Create a default log file path
pub fn default_log_file() -> PathBuf {
    let mut path = if let Some(proj_dirs) = directories::ProjectDirs::from("com", "smart-crawler", "smart-crawler") {
        proj_dirs.data_dir().to_path_buf()
    } else {
        PathBuf::from("./logs")
    };
    
    path.push("crawler.log");
    path
}