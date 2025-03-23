use std::collections::HashSet;
use regex::Regex;
use url::Url;
use tracing::{debug, warn};

use crate::cli::config::{CrawlerSettings, UrlPatterns};

/// Scheduler for determining which URLs should be crawled
pub struct Scheduler {
    /// Configuration for the crawler
    config: CrawlerSettings,
    
    /// Set of already seen URLs to avoid duplicates
    seen_urls: HashSet<String>,
    
    /// Compiled regex patterns for URL inclusion
    include_patterns: Vec<Regex>,
    
    /// Compiled regex patterns for URL exclusion
    exclude_patterns: Vec<Regex>,
    
    /// Allowed domains for crawling (if empty, any domain is allowed)
    allowed_domains: HashSet<String>,
}

impl Scheduler {
    /// Create a new scheduler with the given crawler settings
    pub fn new(config: CrawlerSettings) -> Self {
        // Compile regex patterns for inclusion
        let include_patterns = config.url_patterns.include.iter()
            .filter_map(|pattern| {
                match Regex::new(pattern) {
                    Ok(regex) => Some(regex),
                    Err(e) => {
                        warn!("Invalid include pattern '{}': {}", pattern, e);
                        None
                    }
                }
            })
            .collect();
        
        // Compile regex patterns for exclusion
        let exclude_patterns = config.url_patterns.exclude.iter()
            .filter_map(|pattern| {
                match Regex::new(pattern) {
                    Ok(regex) => Some(regex),
                    Err(e) => {
                        warn!("Invalid exclude pattern '{}': {}", pattern, e);
                        None
                    }
                }
            })
            .collect();
        
        // Create a set of allowed domains
        let allowed_domains = config.allowed_domains.iter()
            .map(|domain| domain.to_lowercase())
            .collect();
        
        Self {
            config,
            seen_urls: HashSet::new(),
            include_patterns,
            exclude_patterns,
            allowed_domains,
        }
    }
    
    /// Determine if a URL should be crawled
    pub fn should_crawl(&mut self, url: &str) -> bool {
        // Normalize the URL
        let normalized_url = self.normalize_url(url);
        
        // Check if we've already seen this URL
        if self.seen_urls.contains(&normalized_url) {
            debug!("Skipping already seen URL: {}", normalized_url);
            return false;
        }
        
        // Parse the URL
        let parsed_url = match Url::parse(&normalized_url) {
            Ok(url) => url,
            Err(e) => {
                debug!("Skipping invalid URL {}: {}", normalized_url, e);
                return false;
            }
        };
        
        // Check if the URL is in an allowed domain
        if !self.allowed_domains.is_empty() {
            if let Some(host) = parsed_url.host_str() {
                let host = host.to_lowercase();
                if !self.allowed_domains.iter().any(|domain| host == *domain || host.ends_with(&format!(".{}", domain))) {
                    debug!("Skipping URL from non-allowed domain: {}", host);
                    return false;
                }
            } else {
                debug!("Skipping URL without host: {}", normalized_url);
                return false;
            }
        }
        
        // Check against exclusion patterns
        for pattern in &self.exclude_patterns {
            if pattern.is_match(&normalized_url) {
                debug!("Skipping URL matching exclusion pattern: {}", normalized_url);
                return false;
            }
        }
        
        // Check against inclusion patterns if any exist
        if !self.include_patterns.is_empty() {
            let mut included = false;
            for pattern in &self.include_patterns {
                if pattern.is_match(&normalized_url) {
                    included = true;
                    break;
                }
            }
            
            if !included {
                debug!("Skipping URL not matching any inclusion pattern: {}", normalized_url);
                return false;
            }
        }
        
        // Add the URL to the seen set
        self.seen_urls.insert(normalized_url);
        
        true
    }
    
    /// Normalize a URL to avoid duplicates due to minor differences
    fn normalize_url(&self, url: &str) -> String {
        // Parse the URL
        let parsed_url = match Url::parse(url) {
            Ok(url) => url,
            Err(_) => return url.to_string(), // Can't normalize, return as is
        };
        
        // Create a new URL with normalized components
        let mut normalized = parsed_url.clone();
        
        // Remove default ports
        if let Some(port) = normalized.port() {
            if (normalized.scheme() == "http" && port == 80) || 
               (normalized.scheme() == "https" && port == 443) {
                let _ = normalized.set_port(None);
            }
        }
        
        // Remove trailing slash
        let path = normalized.path();
        if path == "/" {
            normalized.set_path("");
        }
        
        // Ensure host is lowercase
        if let Some(host) = normalized.host_str() {
            let lowercase_host = host.to_lowercase();
            if host != lowercase_host {
                // This is a bit tricky with the url crate, so we'll just convert to string
                // and parse again if needed
                if let Ok(mut temp_url) = Url::parse(&normalized.to_string().replace(host, &lowercase_host)) {
                    normalized = temp_url;
                }
            }
        }
        
        // Sort query parameters if present
        if let Some(query) = normalized.query() {
            if !query.is_empty() {
                let mut params: Vec<(String, String)> = Vec::new();
                for pair in query.split('&') {
                    let mut kv = pair.split('=');
                    let k = kv.next().unwrap_or("").to_string();
                    let v = kv.next().unwrap_or("").to_string();
                    params.push((k, v));
                }
                
                // Sort params by key
                params.sort_by(|a, b| a.0.cmp(&b.0));
                
                // Rebuild query string
                let sorted_query = params.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<String>>()
                    .join("&");
                
                normalized.set_query(Some(&sorted_query));
            }
        }
        
        // Remove fragments (anchors)
        normalized.set_fragment(None);
        
        normalized.to_string()
    }
    
    /// Get the current count of seen URLs
    pub fn seen_count(&self) -> usize {
        self.seen_urls.len()
    }
    
    /// Clear the seen URLs cache
    pub fn clear_seen(&mut self) {
        self.seen_urls.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::config::{CrawlerSettings, UrlPatterns};
    
    fn create_test_config() -> CrawlerSettings {
        CrawlerSettings {
            max_depth: 3,
            max_pages: 100,
            politeness_delay: 1000,
            respect_robots_txt: true,
            allowed_domains: vec!["example.com".to_string()],
            url_patterns: UrlPatterns {
                include: vec![r"^https?://example\.com/.*$".to_string()],
                exclude: vec![r"^.*\.(jpg|jpeg|png|gif|css|js)$".to_string()],
            },
            user_agent: "TestBot/1.0".to_string(),
        }
    }
    
    #[test]
    fn test_should_crawl() {
        let config = create_test_config();
        let mut scheduler = Scheduler::new(config);
        
        // Should crawl valid URL in allowed domain
        assert!(scheduler.should_crawl("https://example.com/page1"));
        
        // Should not crawl the same URL twice
        assert!(!scheduler.should_crawl("https://example.com/page1"));
        
        // Should not crawl URLs in non-allowed domains
        assert!(!scheduler.should_crawl("https://other-site.com/page"));
        
        // Should not crawl excluded file types
        assert!(!scheduler.should_crawl("https://example.com/image.jpg"));
        
        // Should crawl other valid URLs
        assert!(scheduler.should_crawl("https://example.com/page2"));
    }
    
    #[test]
    fn test_normalize_url() {
        let config = create_test_config();
        let scheduler = Scheduler::new(config);
        
        // Test case insensitivity in host
        assert_eq!(
            scheduler.normalize_url("https://EXAMPLE.com/path"),
            "https://example.com/path"
        );
        
        // Test removal of default ports
        assert_eq!(
            scheduler.normalize_url("https://example.com:443/path"),
            "https://example.com/path"
        );
        
        // Test removal of trailing slash
        assert_eq!(
            scheduler.normalize_url("https://example.com/"),
            "https://example.com"
        );
        
        // Test query parameter sorting
        assert_eq!(
            scheduler.normalize_url("https://example.com/search?b=2&a=1"),
            "https://example.com/search?a=1&b=2"
        );
        
        // Test fragment removal
        assert_eq!(
            scheduler.normalize_url("https://example.com/page#section"),
            "https://example.com/page"
        );
    }
}