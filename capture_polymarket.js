const puppeteer = require('puppeteer');
const fs = require('fs');

(async () => {
  let browser;
  try {
    const possiblePaths = [
      '/usr/bin/google-chrome',
      '/usr/bin/chromium-browser',
      '/usr/bin/chromium',
      '/snap/bin/chromium',
      process.env.CHROME_PATH
    ].filter(Boolean);

    let executablePath = null;
    for (const path of possiblePaths) {
      if (fs.existsSync(path)) {
        executablePath = path;
        console.log('Found browser at: ' + path);
        break;
      }
    }

    const launchOptions = {
      headless: true,
      args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage']
    };
    
    if (executablePath) {
      launchOptions.executablePath = executablePath;
    }

    browser = await puppeteer.launch(launchOptions);
    const page = await browser.newPage();
    await page.setViewport({ width: 1920, height: 1080 });
    
    console.log('Step 1: Navigate to homepage');
    await page.goto('http://localhost:3000/', { waitUntil: 'networkidle0', timeout: 10000 });
    await page.screenshot({ path: 'step1_homepage.png', fullPage: false });
    console.log('Screenshot 1 saved');
    
    console.log('Step 2: Click Polymarket card');
    await page.waitForSelector('.agent-card', { timeout: 5000 });
    const cards = await page.$$('.agent-card');
    for (const card of cards) {
      const text = await card.evaluate(el => el.textContent);
      if (text.includes('Polymarket')) {
        await card.click();
        break;
      }
    }
    await page.waitForTimeout(1000);
    await page.screenshot({ path: 'step2_polymarket_agent.png', fullPage: false });
    console.log('Screenshot 2 saved');
    
    console.log('Step 3: Click Scanner');
    await page.waitForSelector('.sub-tool-card', { timeout: 5000 });
    await page.click('.sub-tool-card');
    await page.waitForTimeout(1500);
    await page.screenshot({ path: 'step3_scanner_page.png', fullPage: false });
    console.log('Screenshot 3 saved');
    
    console.log('Step 4: Scroll to see cards');
    await page.evaluate(() => window.scrollTo(0, 400));
    await page.waitForTimeout(500);
    await page.screenshot({ path: 'step4_scanner_scrolled.png', fullPage: false });
    console.log('Screenshot 4 saved');
    
    console.log('Step 5: Click expand link');
    const expandLink = await page.$('.pm-expand-link');
    if (expandLink) {
      await expandLink.click();
      await page.waitForTimeout(500);
      await page.screenshot({ path: 'step5_expanded_outcomes.png', fullPage: false });
      console.log('Screenshot 5 saved');
    }
    
    console.log('Step 6: Click Strategy button');
    const strategyBtn = await page.$('.pm-btn-strategy');
    if (strategyBtn) {
      await strategyBtn.click();
      await page.waitForTimeout(500);
      await page.screenshot({ path: 'step6_strategy_panel.png', fullPage: false });
      console.log('Screenshot 6 saved');
    }
    
    console.log('Step 7: Click Demo button');
    const demoBtn = await page.$('.pm-btn-demo');
    if (demoBtn) {
      await demoBtn.click();
      await page.waitForTimeout(500);
      await page.screenshot({ path: 'step7_demo_panel.png', fullPage: false });
      console.log('Screenshot 7 saved');
    }
    
    console.log('Step 8: Full page screenshot');
    await page.screenshot({ path: 'step8_full_page.png', fullPage: true });
    console.log('Screenshot 8 saved');
    
    await browser.close();
    console.log('All screenshots captured!');
    
  } catch (error) {
    console.error('Error:', error.message);
    if (browser) await browser.close();
    process.exit(1);
  }
})();
