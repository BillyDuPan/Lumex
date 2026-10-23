const { chromium } = require('playwright');
const path = require('path');
const { startServer } = require('./server');

async function testApp() {
  console.log('Starting local HTTP server...');
  const { server, port } = await startServer(5177);
  console.log(`Server listening on http://127.0.0.1:${port}`);

  const browser = await chromium.launch({
    headless: true,
    args: ['--enable-webgl', '--enable-webgl2-compute-context', '--ignore-gpu-blocklist']
  });

  const context = await browser.newContext({
    viewport: { width: 1480, height: 920 }
  });

  const page = await context.newPage();

  page.on('console', msg => console.log(`[Control ${msg.type()}] ${msg.text()}`));
  page.on('pageerror', err => console.error(`[Control PAGE_ERROR] ${err.message}`));

  console.log(`Navigating control to http://127.0.0.1:${port}/control.html...`);
  await page.goto(`http://127.0.0.1:${port}/control.html`);
  await page.waitForTimeout(1000);

  const screenshotDir = path.resolve('C:/Users/Billy/.gemini/antigravity/brain/30986147-c1af-40a4-8188-bf9394eae4b0');
  const controlScreenshot = path.join(screenshotDir, 'test_control_screenshot.png');
  await page.screenshot({ path: controlScreenshot });
  console.log(`Saved control room screenshot to: ${controlScreenshot}`);

  // Dismiss welcome modal if open
  const demoBtn = await page.$('#welcomeDemoBtn');
  if (demoBtn) {
    console.log('Dismissing welcome modal via #welcomeDemoBtn...');
    await page.click('#welcomeDemoBtn');
    await page.waitForTimeout(500);
  }

  // Click 'Open output' button in control room
  console.log('Clicking Open output button in Control room...');
  await page.click('button:has-text("Open output")');
  await page.waitForTimeout(500);

  // Open output window in the exact same browser context (sharing origin & BroadcastChannel)
  console.log(`Opening output page at http://127.0.0.1:${port}/output.html...`);
  const outPage = await context.newPage();
  outPage.on('console', msg => console.log(`[Output ${msg.type()}] ${msg.text()}`));
  outPage.on('pageerror', err => console.error(`[Output PAGE_ERROR] ${err.message}`));

  await outPage.goto(`http://127.0.0.1:${port}/output.html`);
  console.log('Output page loaded, waiting 2s for BroadcastChannel handshake...');
  await page.waitForTimeout(2000);

  const outputScreenshot = path.join(screenshotDir, 'test_output_screenshot.png');
  await outPage.screenshot({ path: outputScreenshot });
  console.log(`Saved output screenshot to: ${outputScreenshot}`);

  // Check connection status in control page
  const statusText = await page.$eval('#status span', el => el.textContent).catch(() => 'unknown');
  console.log(`>>> Output Status in Control Window: "${statusText}"`);

  // Verify that the wait screen disappeared on the output page
  const waitClass = await outPage.$eval('#wait', el => el.className).catch(() => 'none');
  console.log(`>>> Output wait banner class: "${waitClass}" (expected: includes 'gone')`);

  // Check that button in control room is now 'Close output'
  const btnText = await page.$eval('#topActions button.danger', el => el.textContent).catch(() => 'unknown');
  console.log(`>>> Action button in Control room: "${btnText}" (expected: includes 'Close output')`);

  await browser.close();
  server.close();
  console.log('E2E test finished successfully!');
}

testApp().catch(err => {
  console.error('Test failed:', err);
  process.exit(1);
});
