# Custom script for wikipedia.org
import random
import time
from selenium.webdriver.common.by import By
from selenium.webdriver.support import expected_conditions as EC
from selenium.webdriver.support.ui import WebDriverWait
import re

def crawl(driver, url, behavior, utils):
    """
    Custom crawling logic for Wikipedia
    
    Parameters:
    - driver: Selenium WebDriver instance
    - url: Target URL to crawl
    - behavior: Dictionary of behavior settings
    - utils: BrowserUtils class with helper methods
    
    Returns:
    Dictionary with crawl results including Wikipedia-specific data
    """
    # Navigate to the URL
    driver.get(url)
    
    # Wait for the page to load completely
    utils.random_wait(1.0, 3.0)
    
    # Extract Wikipedia-specific data
    title = driver.title.replace(" - Wikipedia", "")
    
    # Extract article information
    article_data = {}
    try:
        # Get main content
        content_div = driver.find_element(By.ID, "mw-content-text")
        
        # Get first paragraph (usually the summary)
        first_paragraph = content_div.find_element(By.CSS_SELECTOR, ".mw-parser-output > p:not(.mw-empty-elt)")
        article_data["summary"] = first_paragraph.text
        
        # Get infobox if available
        try:
            infobox = driver.find_element(By.CLASS_NAME, "infobox")
            infobox_data = {}
            
            # Get infobox rows
            rows = infobox.find_elements(By.TAG_NAME, "tr")
            for row in rows:
                try:
                    header = row.find_element(By.TAG_NAME, "th")
                    data = row.find_element(By.TAG_NAME, "td")
                    if header.text and data.text:
                        header_text = header.text.strip()
                        data_text = data.text.strip()
                        infobox_data[header_text] = data_text
                except:
                    pass
                    
            article_data["infobox"] = infobox_data
        except:
            pass
        
        # Get sections
        sections = []
        headings = content_div.find_elements(By.CSS_SELECTOR, "h2, h3")
        for heading in headings:
            try:
                # Skip edit links
                if "mw-editsection" in heading.get_attribute("class"):
                    continue
                    
                section_title = heading.text.replace("[edit]", "").strip()
                if section_title and not section_title == "Contents" and not section_title == "References":
                    sections.append(section_title)
            except:
                pass
                
        article_data["sections"] = sections
        
        # Get categories
        categories = []
        try:
            category_links = driver.find_elements(By.CSS_SELECTOR, "#mw-normal-catlinks ul li a")
            for cat in category_links:
                categories.append(cat.text)
        except:
            pass
            
        article_data["categories"] = categories
    except Exception as e:
        print(f"Error extracting Wikipedia-specific data: {e}")
    
    # Simulate human reading behavior
    if behavior.get("scroll_behavior") in ["random", "smooth"]:
        # Scroll down gradually as if reading
        page_height = driver.execute_script("return document.body.scrollHeight")
        view_height = driver.execute_script("return window.innerHeight")
        scrolls = int(page_height / view_height * 0.6)  # Read about 60% of the page
        
        for i in range(min(scrolls, 10)):  # At most 10 scrolls
            utils.scroll(driver, view_height * 0.7)  # Scroll 70% of view height
            utils.random_wait(1.5, 4.0)  # Longer pauses as if reading
    
    # Sometimes interact with references or links
    if random.random() < 0.3:  # 30% chance
        try:
            # Find reference links
            ref_links = driver.find_elements(By.CSS_SELECTOR, ".reference a")
            if ref_links:
                # Click a random reference
                ref_link = random.choice(ref_links)
                utils.human_click(driver, ref_link)
                utils.random_wait(1.0, 2.0)
                
                # Close reference popup if it appeared
                try:
                    close_button = driver.find_element(By.CSS_SELECTOR, ".mwe-popups-close")
                    utils.human_click(driver, close_button)
                except:
                    pass
        except:
            pass
    
    # Sometimes search for something related
    if random.random() < 0.2:  # 20% chance
        try:
            # Get search box
            search_box = driver.find_element(By.ID, "searchInput")
            
            # Generate related search query based on title
            words = re.findall(r'\w+', title)
            if words:
                search_term = random.choice(words)
                if len(search_term) > 3:  # Only search for meaningful words
                    utils.human_type(driver, search_box, search_term)
                    utils.random_wait(0.5, 1.0)
                    
                    # Click search button
                    search_button = driver.find_element(By.ID, "searchButton")
                    utils.human_click(driver, search_button)
                    
                    # Wait for search results
                    utils.random_wait(2.0, 3.0)
                    
                    # Go back to the original page
                    driver.back()
                    utils.random_wait(1.0, 2.0)
        except:
            pass
    
    # Extract links for the crawler
    links = utils.extract_all_links(driver)
    
    # Filter links to prioritize Wikipedia articles
    filtered_links = []
    for link in links:
        url = link["url"]
        # Prioritize Wikipedia article links, exclude special pages
        if "wikipedia.org/wiki/" in url and not any(x in url for x in [
            "Special:", "Talk:", "User:", "Wikipedia:", "Help:", "File:", "Template:"
        ]):
            filtered_links.append(link)
    
    return {
        "title": title,
        "content": driver.page_source,
        "links": [link["url"] for link in filtered_links],
        "article_data": article_data,
        "metrics": utils.get_page_metrics(driver)
    }