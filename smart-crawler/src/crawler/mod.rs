pub mod controller;
pub mod scheduler;
pub mod task;

// Re-export common types
pub use controller::CrawlerController;
pub use task::{CrawlTask, TaskResult, TaskError};
pub use scheduler::Scheduler;