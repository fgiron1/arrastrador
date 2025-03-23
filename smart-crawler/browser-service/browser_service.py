from flask import Flask, request, jsonify
from selenium import webdriver
from selenium.webdriver.chrome.service import Service as ChromeService
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.firefox.service import Service as FirefoxService
from selenium.webdriver.firefox.options import Options as FirefoxOptions
from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait
from selenium.webdriver.support import expected_conditions as EC
from selenium.common.exceptions import WebDriverException, TimeoutException
from selenium.webdriver.common.action_chains import ActionChains
from webdriver_manager.chrome import ChromeDriverManager
from webdriver_manager.firefox import GeckoDriverManager
import random
import time
import os
import json
import logging
import traceback
import importlib.util
import sys
from pathlib import Path

# Configure logging
logging.basicConfig(level=logging.INFO, 
                    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s')
logger = logging.getLogger('browser-service')

app = Flask(__name__)

# Set up driver paths
DRIVER_DIR = os.environ.get('DRIVER_DIR', os.path.join(os.path.dirname(__file__), 'drivers'))
CHROME_DRIVER_PATH = os.path.join(DRIVER_DIR, 'chromedriver')
FIREFOX_DRIVER_PATH = os.path.join(DRIVER_DIR, 'geckodriver')

# Custom scripts directory
SCRIPTS_DIR = os.environ.get('SCRIPTS_DIR', os.path.join(os.path.dirname(__file__), 'scripts'))
os.makedirs(SCRIPTS_DIR, exist_ok=True)

# Initialize pools
driver_pools = {
    'chrome': [],
    'firefox': []
}

MAX_POOL_SIZE = 5

class BrowserUtils:
    """Utility class with helpful browser automation functions"""
    
    @staticmethod
    def configure_driver(browser_type, fingerprint):
        """Configure a browser with specified fingerprint settings"""
        if browser_type == 'chrome':
            options = Options()
            
            # Set user agent if provided
            if 'user_agent' in fingerprint:
                options.add_argument(f"user-agent={fingerprint['user_agent']}")
            
            # Anti-detection measures
            options.add_argument('--disable-blink-features=AutomationControlled')
            options.add_experimental_option('excludeSwitches', ['enable-automation'])
            options.add_experimental_option('useAutomationExtension', False)
            
            # Set language
            if 'accept_language' in fingerprint:
                options.add_argument(f"--lang={fingerprint['accept_language'].split(',')[0]}")
            
            # Set viewport
            if 'viewport' in fingerprint:
                viewport = fingerprint['viewport']
                options.add_argument(f"--window-size={viewport['width']},{viewport['height']}")
            
            # Add proxy if specified
            if 'proxy' in fingerprint:
                options.add_argument(f"--proxy-server={fingerprint['proxy']}")
            
            # Set headless mode
            if os.environ.get('HEADLESS', 'true').lower() == 'true':
                options.add_argument('--headless')
            
            # Additional privacy/fingerprinting prevention
            options.add_argument('--disable-dev-shm-usage')
            options.add_argument('--no-sandbox')
            
            try:
                service = ChromeService(executable_path=ChromeDriverManager().install())
                driver = webdriver.Chrome(service=service, options=options)
                
                # Apply anti-fingerprinting script
                driver.execute_script("""
                    Object.defineProperty(navigator, 'webdriver', {
                        get: () => undefined
                    });
                """)
                
                # Set additional navigator properties if provided
                if 'platform' in fingerprint:
                    driver.execute_script(f"""
                        Object.defineProperty(navigator, 'platform', {{
                            get: () => '{fingerprint['platform']}'
                        }});
                    """)
                
                if 'hardware_concurrency' in fingerprint:
                    driver.execute_script(f"""
                        Object.defineProperty(navigator, 'hardwareConcurrency', {{
                            get: () => {fingerprint['hardware_concurrency']}
                        }});
                    """)
                
                return driver
            except Exception as e:
                logger.error(f"Failed to create Chrome driver: {str(e)}")
                raise
                
        elif browser_type == 'firefox':
            options = FirefoxOptions()
            
            if 'user_agent' in fingerprint:
                options.set_preference("general.useragent.override", fingerprint['user_agent'])
            
            if 'accept_language' in fingerprint:
                options.set_preference("intl.accept_languages", fingerprint['accept_language'])
            
            if os.environ.get('HEADLESS', 'true').lower() == 'true':
                options.add_argument('--headless')
            
            try:
                service = FirefoxService(executable_path=GeckoDriverManager().install())
                driver = webdriver.Firefox(service=service, options=options)
                return driver
            except Exception as e:
                logger.error(f"Failed to create Firefox driver: {str(e)}")
                raise
        
        raise ValueError(f"Unsupported browser type: {browser_type}")
    
    @staticmethod
    def scroll(driver, amount=None, behavior='smooth', direction='down'):
        """Scroll the page in a human-like manner"""
        if amount is None:
            amount = random.randint(300, 800)
            
        if direction == 'up':
            amount = -amount
            
        driver.execute_script(
            f"window.scrollBy({{ top: {amount}, left: 0, behavior: '{behavior}' }});"
        )
    
    @staticmethod
    def human_click(driver, element):
        """Click an element in a human-like way with hover first"""
        if element.is_displayed():
            actions = ActionChains(driver)
            actions.move_to_element(element)
            actions.pause(random.uniform(0.1, 0.5))
            actions.click()
            actions.perform()
            return True
        return False
    
    @staticmethod
    def human_type(driver, element, text, min_delay=0.05, max_delay=0.25):
        """Type text with human-like delays between keystrokes"""
        if not element.is_displayed():
            return False
            
        try:
            element.clear()
            for char in text:
                element.send_keys(char)
                time.sleep(random.uniform(min_delay, max_delay))
            return True
        except Exception as e:
            logger.warning(f"Error typing text: {e}")
            return False
    
    @staticmethod
    def random_wait(min_sec=0.5, max_sec=3.0):
        """Wait for a random period within a specified range"""
        time.sleep(random.uniform(min_sec, max_sec))
    
    @staticmethod
    def find_clickable_elements(driver, max_elements=10):
        """Find a list of potentially clickable elements on the page"""
        clickable = []
        
        # Links
        for elem in driver.find_elements(By.TAG_NAME, 'a'):
            if elem.is_displayed() and elem.is_enabled():
                clickable.append(elem)
                if len(clickable) >= max_elements:
                    return clickable
                    
        # Buttons
        for elem in driver.find_elements(By.TAG_NAME, 'button'):
            if elem.is_displayed() and elem.is_enabled():
                clickable.append(elem)
                if len(clickable) >= max_elements:
                    return clickable
        
        # Input buttons
        for elem in driver.find_elements(By.XPATH, "//input[@type='button' or @type='submit']"):
            if elem.is_displayed() and elem.is_enabled():
                clickable.append(elem)
                if len(clickable) >= max_elements:
                    return clickable
                    
        return clickable
    
    @staticmethod
    def find_forms(driver):
        """Find all forms and their input fields on the page"""
        forms = []
        for form in driver.find_elements(By.TAG_NAME, 'form'):
            input_elements = {}
            
            # Text inputs
            for inp in form.find_elements(By.XPATH, ".//input[@type='text' or @type='search' or @type='email' or @type='password']"):
                if inp.is_displayed():
                    input_type = inp.get_attribute('type')
                    name = inp.get_attribute('name') or inp.get_attribute('id') or f"input_{len(input_elements)}"
                    input_elements[name] = {
                        'element': inp,
                        'type': input_type
                    }
            
            # Submit buttons
            submits = []
            for btn in form.find_elements(By.XPATH, ".//button[@type='submit'] | .//input[@type='submit']"):
                if btn.is_displayed():
                    submits.append(btn)
            
            forms.append({
                'form': form,
                'inputs': input_elements,
                'submit_buttons': submits
            })
            
        return forms
    
    @staticmethod
    def get_page_metrics(driver):
        """Collect various metrics about the page"""
        return driver.execute_script("""
            return {
                'title': document.title,
                'url': window.location.href,
                'domain': window.location.hostname,
                'links_count': document.getElementsByTagName('a').length,
                'images_count': document.getElementsByTagName('img').length,
                'forms_count': document.getElementsByTagName('form').length,
                'scripts_count': document.getElementsByTagName('script').length,
                'height': document.body.scrollHeight,
                'width': document.body.scrollWidth
            }
        """)
    
    @staticmethod
    def extract_all_links(driver):
        """Extract all links from the page with their text content"""
        links = []
        for link in driver.find_elements(By.TAG_NAME, 'a'):
            try:
                href = link.get_attribute('href')
                if href and href.startswith('http'):
                    links.append({
                        'url': href,
                        'text': link.text.strip(),
                        'visible': link.is_displayed()
                    })
            except:
                pass
        return links


def get_driver(browser_type, fingerprint):
    """Get a driver from the pool or create a new one"""
    if browser_type in driver_pools and driver_pools[browser_type]:
        driver = driver_pools[browser_type].pop()
        logger.info(f"Reusing {browser_type} driver from pool")
        return driver
    
    logger.info(f"Creating new {browser_type} driver")
    return BrowserUtils.configure_driver(browser_type, fingerprint)


def return_driver_to_pool(browser_type, driver):
    """Return a driver to the pool if there's room, otherwise quit it"""
    if browser_type in driver_pools and len(driver_pools[browser_type]) < MAX_POOL_SIZE:
        driver_pools[browser_type].append(driver)
        logger.info(f"Returned {browser_type} driver to pool")
    else:
        driver.quit()
        logger.info(f"Driver quit: {browser_type}")


def load_custom_script(domain):
    """Load a custom script for the specific domain if available"""
    domain = domain.replace('.', '_').replace('-', '_')
    script_path = os.path.join(SCRIPTS_DIR, f"{domain}.py")
    
    if os.path.exists(script_path):
        logger.info(f"Loading custom script for domain: {domain}")
        
        try:
            # Load the module
            spec = importlib.util.spec_from_file_location(domain, script_path)
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
            
            # Check if it has the required function
            if hasattr(module, 'crawl'):
                return module
            else:
                logger.warning(f"Custom script {domain}.py exists but doesn't have a crawl function")
        except Exception as e:
            logger.error(f"Error loading custom script {domain}.py: {e}")
    
    return None


def default_crawl(driver, url, behavior):
    """Default crawling behavior when no custom script exists"""
    # Navigate to URL
    driver.get(url)
    
    # Wait for page to load
    wait_time = random.uniform(1, 3)
    time.sleep(wait_time)
    
    # Get page title and metrics
    title = driver.title
    metrics = BrowserUtils.get_page_metrics(driver)
    
    # Perform some scrolling
    scroll_count = random.randint(1, 5)
    for _ in range(scroll_count):
        BrowserUtils.scroll(driver)
        BrowserUtils.random_wait(0.5, 2.0)
    
    # Hover over some elements
    if behavior.get('mouse_movement', False):
        clickable = BrowserUtils.find_clickable_elements(driver, 5)
        for element in clickable[:2]:  # Just hover over first 2
            try:
                actions = ActionChains(driver)
                actions.move_to_element(element)
                actions.perform()
                BrowserUtils.random_wait(0.2, 1.0)
            except:
                pass
    
    # Extract data
    links = BrowserUtils.extract_all_links(driver)
    content = driver.page_source
    
    return {
        'title': title,
        'content': content,
        'links': [link['url'] for link in links],
        'metrics': metrics
    }


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
        # Get the domain from the URL
        from urllib.parse import urlparse
        domain = urlparse(url).netloc
        
        # Get a driver
        driver = get_driver(browser_type, fingerprint)
        
        # Set page load timeout
        driver.set_page_load_timeout(30)
        
        # Try to load a custom script for this domain
        custom_script = load_custom_script(domain)
        
        if custom_script:
            # Use the custom crawling logic
            logger.info(f"Using custom crawl script for {domain}")
            result = custom_script.crawl(driver, url, behavior, BrowserUtils)
        else:
            # Use default crawling behavior
            logger.info(f"Using default crawl behavior for {domain}")
            result = default_crawl(driver, url, behavior)
        
        # Take screenshot if requested
        screenshot = None
        if data.get('take_screenshot', False):
            screenshot = driver.get_screenshot_as_base64()
        
        # Return driver to pool
        return_driver_to_pool(browser_type, driver)
        driver = None
        
        # Prepare response
        response = {
            'success': True,
            'url': url,
            'title': result.get('title', ''),
            'content': result.get('content', ''),
            'links': result.get('links', []),
            'screenshot': screenshot,
            'metrics': result.get('metrics', {})
        }
        
        return jsonify(response)
        
    except WebDriverException as e:
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
        'pool_sizes': {k: len(v) for k, v in driver_pools.items()},
        'custom_scripts': [f.stem for f in Path(SCRIPTS_DIR).glob('*.py')]
    })


@app.route('/script/<domain>', methods=['PUT'])
def upload_script(domain):
    """Endpoint to upload a custom script for a domain"""
    if not request.is_json:
        return jsonify({'success': False, 'error': 'Expected JSON content'})
    
    script_content = request.json.get('script')
    if not script_content:
        return jsonify({'success': False, 'error': 'No script content provided'})
    
    # Sanitize domain for filename
    safe_domain = domain.replace('.', '_').replace('-', '_')
    script_path = os.path.join(SCRIPTS_DIR, f"{safe_domain}.py")
    
    try:
        with open(script_path, 'w') as f:
            f.write(script_content)
        
        return jsonify({'success': True, 'message': f'Script for {domain} uploaded successfully'})
    except Exception as e:
        return jsonify({'success': False, 'error': f'Error saving script: {str(e)}'})


if __name__ == '__main__':
    # Initialize empty pools
    for browser in driver_pools:
        driver_pools[browser] = []
    
    logger.info(f"Custom scripts directory: {SCRIPTS_DIR}")
    
    # Run the Flask app
    app.run(host='0.0.0.0', port=5000, threaded=True)