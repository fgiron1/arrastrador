# Smart Crawler

A modern, undetectable web crawler designed to mimic human behavior and avoid detection.

## Features

- **Undetectable**: Employs multiple strategies to avoid crawler detection
- **Distributed**: Runs on Kubernetes for scalability
- **Human-like Behavior**: Simulates realistic browsing patterns
- **Fingerprint Rotation**: Varies browser fingerprints to avoid tracking
- **IP Rotation**: Uses proxies and VPNs to distribute requests
- **Configurable**: Extensive YAML-based configuration system
- **Efficient Storage**: Stores raw and processed data separately
- **Data Export**: Exports crawled data in multiple formats (JSON, CSV, SQL)

## Prerequisites

- Rust 1.65 or later
- Redis (for task queue)
- MongoDB (for raw data storage)
- PostgreSQL (for processed data storage)
- Chrome/Chromium with WebDriver
- Kubernetes cluster (optional, for distributed operation)

## Installation

1. Clone this repository:
   ```bash
   git clone https://github.com/yourusername/smart-crawler.git
   cd smart-crawler
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

3. Install the CLI globally:
   ```bash
   cargo install --path .
   ```

## Quick Start

1. Start a new crawling job:
   ```bash
   crawler crawl https://example.com --profile general
   ```

2. Check the status of a job:
   ```bash
   crawler status <job-id>
   ```

3. Export crawled data:
   ```bash
   crawler export <job-id> --format json --output data.json
   ```

## Configuration

The crawler uses YAML configuration files located in the `~/.config/smart-crawler/` directory. 
You can create different profiles for different websites or crawling strategies.

Example configuration:

```yaml
crawler:
  max_depth: 3
  max_pages: 1000
  politeness_delay: 2000
  respect_robots_txt: true
  allowed_domains:
    - example.com
  url_patterns:
    include:
      - "^https?://example\\.com/.*$"
    exclude:
      - "^.*\\.(jpg|jpeg|png|gif|css|js)$"

browser:
  browser_type: chrome
  headless: true
  viewport:
    width: 1920
    height: 1080
    device_scale_factor: 1.0
  fingerprints:
    - name: windows_chrome
      user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36"
      accept_language: "en-US,en;q=0.9"
      platform: "Win32"
  behavior:
    scroll_behavior: random
    click_delay: [100, 300]
    typing_speed: [50, 150]
    mouse_movement: true
    session_duration: [300, 1800]

proxy:
  enabled: true
  rotation_strategy: session
  rotation_interval: 600
  proxy_list:
    - name: proxy1
      proxy_type: http
      address: proxy1.example.com
      port: 8080
    - name: proxy2
      proxy_type: socks5
      address: proxy2.example.com
      port: 1080
```

## Kubernetes Deployment

For distributed crawling, deploy to Kubernetes:

```bash
kubectl apply -f kubernetes/controller.yaml
kubectl apply -f kubernetes/worker.yaml
kubectl apply -f kubernetes/storage.yaml
```

## License

MIT License

## Disclaimer

This tool is for educational and research purposes only. Always respect websites' terms of service and robots.txt files. The authors are not responsible for any misuse of this software.