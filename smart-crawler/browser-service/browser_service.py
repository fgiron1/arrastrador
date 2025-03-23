from flask import Flask, request, jsonify
from selenium import webdriver
from selenium.webdriver.chrome.service import Service as ChromeService
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.firefox.service import Service as FirefoxService
from selenium.webdriver.firefox.options import Options as FirefoxOptions
from selenium.common.exceptions import WebDriverException
import random
import time
import os
import json
import logging

# Configure logging
logging.basicConfig(level=logging.INFO, 
                    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s')
logger = logging.getLogger('browser-service')

app = Flask(__name__)

# Set up driver paths - directly in the service's directory
DRIVER_DIR = os.environ.get('DRIVER_DIR', os.path.join(os.path.dirname(__file__), 'drivers'))
CHROME_DRIVER_PATH = os.path.join(DRIVER_DIR, 'chromedriver')
FIREFOX_DRIVER_PATH = os.path.join(DRIVER_DIR, 'geckodriver')
IE_DRIVER_PATH = os.path.join(DRIVER_DIR, 'IEDriverServer')
OPERA_DRIVER_PATH = os.path.join(DRIVER_DIR, 'operadriver')

logger.info(f"Using driver directory: {DRIVER_DIR}")
logger.info(f"Chrome driver path: {CHROME_DRIVER_PATH}")
logger.info(f"Firefox driver path: {FIREFOX_DRIVER_PATH}")

# Create driver pools for reuse
driver_pools = {
    'chrome': [],
    'firefox': [],
    'ie': [],
    'opera': []
}

MAX_POOL_SIZE = 5  # Maximum number of drivers per browser type

def get_driver(browser_type, fingerprint):
    """Get a driver from the pool or create a new one"""
    if driver_pools[browser_type] and len(driver_pools[browser_type]) > 0:
        # Reuse an existing driver
        driver = driver_pools[browser_type].pop()
        logger.info(f"Reusing {browser_type} driver from pool")
        return driver
    
    # Create a new driver
    logger.info(f"Creating new {browser_type} driver")
    if browser_type == 'chrome':
        options = Options()
        if fingerprint.get('user_agent'):
            options.add_argument(f"user-agent={fingerprint['user_agent']}")
        
        options.add_argument('--disable-blink-features=AutomationControlled')
        options.add_experimental_option('excludeSwitches', ['enable-automation'])
        options.add_experimental_option('useAutomationExtension', False)
        
        # Add proxy if specified
        if fingerprint.get('proxy'):
            options.add_argument(f"--proxy-server={fingerprint['proxy']}")
        
        try:
            service = ChromeService(ChromeDriverManager().install())
            driver = webdriver.Chrome(service=service, options=options)
            
            # Apply anti-fingerprinting script
            driver.execute_script("""
                Object.defineProperty(navigator, 'webdriver', {
                    get: () => undefined
                });
            """)
            
            return driver
        except Exception as e:
            logger.error(f"Failed to create Chrome driver: {str(e)}")
            raise
            
    elif browser_type == 'firefox':
        options = FirefoxOptions()
        if fingerprint.get('user_agent'):
            options.set_preference("general.useragent.override", fingerprint['user_agent'])
        
        # Add proxy if specified
        if fingerprint.get('proxy'):
            proxy_parts = fingerprint['proxy'].split(':')
            if len(proxy_parts) >= 2:
                options.set_preference("network.proxy.type", 1)
                options.set_preference("network.proxy.http", proxy_parts[0])
                options.set_preference("network.proxy.http_port", int(proxy_parts[1]))
                options.set_preference("network.proxy.ssl", proxy_parts[0])
                options.set_preference("network.proxy.ssl_port", int(proxy_parts[1]))
        
        try:
            service = FirefoxService(GeckoDriverManager().install())
            driver = webdriver.Firefox(service=service, options=options)
            return driver
        except Exception as e:
            logger.error(f"Failed to create Firefox driver: {str(e)}")
            raise
    
    # Implement other browser drivers as needed
    
    raise ValueError(f"Unsupported browser type: {browser_type}")

def return_driver_to_pool(browser_type, driver):
    """Return a driver to the pool if there's room, otherwise quit it"""
    if len(driver_pools[browser_type]) < MAX_POOL_SIZE:
        driver_pools[browser_type].append(driver)
        logger.info(f"Returned {browser_type} driver to pool")
    else:
        driver.quit()
        logger.info(f"Pool full, quitting {browser_type} driver")

def simulate_behavior(driver, behavior):
    """Simulate human-like browsing behavior"""
    try:
        # Random scrolling
        if behavior.get('scroll_behavior') in ['random', 'smooth']:
            scroll_amount = random.randint(300, 1000)
            scroll_behavior = behavior.get('scroll_behavior', 'auto')
            driver.execute_script(
                f"window.scrollBy({{ top: {scroll_amount}, left: 0, behavior: '{scroll_behavior}' }});"
            )
            time.sleep(random.uniform(0.5, 2.0))
        
        # Random pauses
        time.sleep(random.uniform(1.0, 3.0))
        
        # Mouse movement simulation (basic)
        if behavior.get('mouse_movement', False):
            # Just move to a random element to simulate mouse movement
            elements = driver.find_elements_by_tag_name('a')
            if elements and len(elements) > 0:
                try:
                    rand_idx = random.randint(0, min(10, len(elements) - 1))
                    webdriver.ActionChains(driver).move_to_element(elements[rand_idx]).perform()
                except:
                    pass
        
        # More advanced behavior can be added based on your behavior settings
    except Exception as e:
        logger.warning(f"Error during behavior simulation: {str(e)}")

def extract_page_data(driver):
    """Extract data from the page"""
    try:
        # Get page source
        content = driver.page_source
        
        # Extract links
        links = []
        for link in driver.find_elements_by_tag_name('a'):
            try:
                href = link.get_attribute('href')
                if href and href.startswith('http'):
                    links.append(href)
            except:
                pass
        
        return {
            'content': content,
            'links': links
        }
    except Exception as e:
        logger.error(f"Error extracting page data: {str(e)}")
        return {'content': '', 'links': []}

@app.route('/crawl', methods=['POST'])
def crawl():
    data = request.json
    url = data['url']
    browser_type = data.get('browser_type', 'chrome').lower()
    fingerprint = data.get('fingerprint', {})
    behavior = data.get('behavior', {})
    
    if browser_type not in driver_pools:
        return jsonify({
            'success': False,
            'error': f"Unsupported browser type: {browser_type}"
        })
    
    driver = None
    try:
        # Get a driver
        driver = get_driver(browser_type, fingerprint)
        
        # Set page load timeout
        driver.set_page_load_timeout(30)
        
        # Navigate to URL
        driver.get(url)
        
        # Get page title
        title = driver.title
        
        # Simulate human behavior
        simulate_behavior(driver, behavior)
        
        # Extract data
        page_data = extract_page_data(driver)
        
        # Take screenshot if requested
        screenshot = None
        if data.get('take_screenshot', False):
            screenshot = driver.get_screenshot_as_base64()
        
        # Return driver to pool instead of quitting
        return_driver_to_pool(browser_type, driver)
        driver = None
        
        return jsonify({
            'success': True,
            'url': url,
            'title': title,
            'content': page_data['content'],
            'links': page_data['links'],
            'screenshot': screenshot
        })
        
    except WebDriverException as e:
        # Handle WebDriver specific exceptions
        error_msg = f"WebDriver error for {url}: {str(e)}"
        logger.error(error_msg)
        return jsonify({
            'success': False,
            'error': error_msg,
            'url': url,
            'title': '',
            'content': '',
            'links': []
        })
        
    except Exception as e:
        # Handle general exceptions
        error_msg = f"Error crawling {url}: {str(e)}\n{traceback.format_exc()}"
        logger.error(error_msg)
        return jsonify({
            'success': False,
            'error': error_msg,
            'url': url,
            'title': '',
            'content': '',
            'links': []
        })
        
    finally:
        # Make sure to clean up if something went wrong
        if driver:
            try:
                return_driver_to_pool(browser_type, driver)
            except:
                driver.quit()

@app.route('/health', methods=['GET'])
def health_check():
    """Simple health check endpoint"""
    return jsonify({
        'status': 'ok',
        'pool_sizes': {k: len(v) for k, v in driver_pools.items()}
    })

if __name__ == '__main__':
    # Initialize empty pools
    for browser in driver_pools:
        driver_pools[browser] = []
    
    # Run the Flask app
    app.run(host='0.0.0.0', port=5000, threaded=True)