// src/browser/mod.rs
pub mod fingerprint;
pub mod remote;
pub mod script;

// Re-export common types
pub use fingerprint::FingerprintManager;
pub use remote::RemoteBrowserService;
pub use script::ScriptManager;