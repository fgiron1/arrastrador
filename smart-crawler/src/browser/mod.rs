// src/browser/mod.rs
pub mod behavior;
pub mod fingerprint;
pub mod session;
pub mod remote;  // Add this line

// Re-export common types
pub use behavior::BehaviorSimulator;
pub use fingerprint::{FingerprintManager, CompleteFingerprint};
pub use session::BrowserSession;
pub use remote::RemoteBrowserService;  // Add this line