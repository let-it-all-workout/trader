const puppeteer = require('puppeteer-core');

(async () => {
  const browser = await puppeteer.launch({
    executablePath: '/usr/bin/firefox',
    headless: true,
    product: 'firefox'
  });
  
  const page = await browser.newPage();
  await page.setViewport({ width: 1920, height: 1080 });
  
  console.log('1. Homepage...');
  await page.goto('http://localhost:3000/', { waitUntil: 'networkidle0' });
  await page.screenshot({ path: 'step1_homepage.png' });
  
  console.log('2. Click Polymarket...');
  await page.waitForTimeout(500);
  const cards = await page.$$('.agent-card');
  for (const card of cards) {
    const text = await card.evaluate(el => el.textContent);
    if (text.includes('Polymarket')) {
      await card.click();
      break;
    }
  }
  await page.waitForTimeout(1500);
  await page.screenshot({ path: 'step2_agent.png' });
  
  console.log('3. Click Scanner...');
  await page.click('.sub-tool-card');
  await page.waitForTimeout(2000);
  await page.screenshot({ path: 'step3_scanner.png' });
  
  console.log('4. Scroll...');
  await page.evaluate(() => window.scrollTo(0, 400));
  await page.waitForTimeout(500);
  await page.screenshot({ path: 'step4_scrolled.png' });
  
  console.log('5. Expand...');
  const expand = await page.$('.pm-expand-link');
  if (expand) {
    await expand.click();
    await page.waitForTimeout(500);
    await page.screenshot({ path: 'step5_expanded.png' });
  }
  
  console.log('6. Strategy...');
  const strategy = await page.$('.pm-btn-strategy');
  if (strategy) {
    await strategy.click();
    await page.waitForTimeout(500);
    await page.screenshot({ path: 'step6_strategy.png' });
  }
  
  console.log('7. Demo...');
  const demo = await page.$('.pm-btn-demo');
  if (demo) {
    await demo.click();
    await page.waitForTimeout(500);
    await page.screenshot({ path: 'step7_demo.png' });
  }
  
  console.log('8. Full page...');
  await page.screenshot({ path: 'step8_full.png', fullPage: true });
  
  await browser.close();
  console.log('Done!');
})().catch(console.error);
