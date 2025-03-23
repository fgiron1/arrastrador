pub mod manager;
pub mod vpn;

// Re-export common types
pub use manager::ProxyManager;
pub use vpn::VpnManager;