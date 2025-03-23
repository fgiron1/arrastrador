pub mod logging;
pub mod metrics;

// Re-export common functions and types
pub use logging::{init_logging, default_log_file};
pub use metrics::{MetricsCollector, Metrics, RequestTimer};