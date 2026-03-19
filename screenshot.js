const puppeteer = require('puppeteer');

(async () => {
  const browser = await puppeteer.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox']
  });
  
  const page = await browser.newPage();
  await page.setViewport({ width: 1920, height: 1080 });
  
  // Screenshot 1: Dashboard
  console.log('Navigating to dashboard...');
  await page.goto('http://localhost:3000/', { waitUntil: 'networkidle0' });
  await page.screenshot({ path: 'screenshot1_dashboard.png', fullPage: true });
  console.log('✓ Screenshot 1: Dashboard saved');
  
  // Screenshot 2: Funding Rate page
  console.log('Navigating to funding rate page...');
  await page.goto('http://localhost:3000/scanner/funding', { waitUntil: 'networkidle0' });
  await page.screenshot({ path: 'screenshot2_funding_top.png', fullPage: false });
  console.log('✓ Screenshot 2: Funding Rate (top) saved');
  
  // Screenshot 3: Scroll down to see table
  console.log('Scrolling to table...');
  await page.evaluate(() => window.scrollTo(0, 800));
  await page.waitForTimeout(500);
  await page.screenshot({ path: 'screenshot3_funding_table.png', fullPage: false });
  console.log('✓ Screenshot 3: Funding Rate (table) saved');
  
  // Screenshot 4: Full page
  await page.screenshot({ path: 'screenshot4_funding_full.png', fullPage: true });
  console.log('✓ Screenshot 4: Funding Rate (full page) saved');
  
  await browser.close();
  console.log('\n✅ All screenshots captured successfully!');
})();
