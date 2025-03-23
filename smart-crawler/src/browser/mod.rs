pub mod behavior;
pub mod fingerprint;
pub mod session;

// Re-export common types
pub use behavior::BehaviorSimulator;
pub use fingerprint::{FingerprintManager, CompleteFingerprint};
pub use session::BrowserSession;