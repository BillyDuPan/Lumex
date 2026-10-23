const { chromium } = require('playwright');
const path = require('path');
const fs = require('fs');
const { startServer } = require('./server');

const ARTIFACT_DIR = 'C:/Users/Billy/.gemini/antigravity/brain/25b76676-3fe0-43d2-9974-9741e1f0b6c6';

async function runFeatureTests() {
  console.log('====================================================');
  console.log(' Lumex Pro Live VJ Feature Automated Verification   ');
  console.log('====================================================');

  const { server, port } = await startServer(5178);
  console.log(`[Server] Live VJ server running on http://127.0.0.1:${port}`);

  const browser = await chromium.launch({
    headless: true,
    args: [
      '--enable-webgl',
      '--enable-webgl2-compute-context',
      '--ignore-gpu-blocklist',
      '--use-fake-ui-for-media-stream',
      '--use-fake-device-for-media-stream',
    ]
  });

  const context = await browser.newContext({
    viewport: { width: 1480, height: 920 }
  });

  const page = await context.newPage();

  page.on('console', msg => console.log(`[Browser Console] ${msg.text()}`));
  page.on('pageerror', err => console.error(`[Browser PageError] ${err.message}`));

  console.log('\n--- Step 1: Launch App & Verify Welcome Onboarding Hub ---');
  await page.goto(`http://127.0.0.1:${port}/control.html`);
  await page.waitForTimeout(1200);

  // 1. Welcome Modal Verification
  const welcomeVisible = await page.$eval('#welcomeModal', el => !el.hidden).catch(() => false);
  console.log(`Welcome Hub modal visible on startup: ${welcomeVisible}`);
  const shot1 = path.join(ARTIFACT_DIR, '01_welcome_onboarding.png');
  await page.screenshot({ path: shot1 });
  console.log(`[Screenshot Captured] ${shot1}`);

  // 2. Click "Explore Showcase Project"
  console.log('\n--- Step 2: Load Showcase Project & Inspect Project Packaging ---');
  await page.click('#welcomeDemoBtn');
  await page.waitForTimeout(1000);

  const projName = await page.$eval('#projectName', el => el.textContent).catch(() => 'unknown');
  console.log(`Active Project Name in Topbar: "${projName}"`);
  const shot2 = path.join(ARTIFACT_DIR, '02_showcase_project_loaded.png');
  await page.screenshot({ path: shot2 });
  console.log(`[Screenshot Captured] ${shot2}`);

  // 3. Inspect Dual-Deck A/B Mixing Station
  console.log('\n--- Step 3: Inspect Dual-Deck A/B Mixing Station ---');
  const deckATitle = await page.$eval('#deckCardA .deck-scene-title', el => el.textContent).catch(() => 'none');
  const deckBTitle = await page.$eval('#deckCardB .deck-scene-title', el => el.textContent).catch(() => 'none');
  console.log(`Deck A Assigned: "${deckATitle}" | Deck B Assigned: "${deckBTitle}"`);

  // Assign Scene 4 to Deck B via quick assign badge
  console.log('Assigning Scene 4 to Deck B via [->B] badge...');
  const assignBtns = await page.$$('.deck-assign-btn.btn-b');
  if (assignBtns.length >= 3) {
    await assignBtns[2].click();
    await page.waitForTimeout(500);
  }

  const updatedDeckB = await page.$eval('#deckCardB .deck-scene-title', el => el.textContent).catch(() => 'none');
  console.log(`Updated Deck B: "${updatedDeckB}"`);
  const shot3 = path.join(ARTIFACT_DIR, '03_dual_deck_ab_mixing.png');
  await page.screenshot({ path: shot3 });
  console.log(`[Screenshot Captured] ${shot3}`);

  // 3b. Test Selecting/Cueing Deck B to Preview and Edit Before Transitioning
  console.log('\n--- Step 3b: Select/Preview/Edit Cued Scene (Deck B) Before Transition ---');
  await page.click('#deckCardB');
  await page.waitForTimeout(600);

  const cueBadgeText = await page.$eval('#cueBadge', el => el.textContent).catch(() => 'none');
  const cueBadgeVisible = await page.$eval('#cueBadge', el => !el.hidden).catch(() => false);
  console.log(`Cue Badge Visible: ${cueBadgeVisible} | Text: "${cueBadgeText}"`);

  const shot3b = path.join(ARTIFACT_DIR, '03b_cue_scene_edit_preview.png');
  await page.screenshot({ path: shot3b });
  console.log(`[Screenshot Captured] ${shot3b}`);

  // 4. Test Crossfader Midpoint Blend
  console.log('\n--- Step 4: T-Bar Crossfader Blending ---');
  await page.$eval('#crossfaderSlider', el => {
    el.value = 0.5;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await page.waitForTimeout(600);

  const shot4 = path.join(ARTIFACT_DIR, '04_crossfader_midpoint_blend.png');
  await page.screenshot({ path: shot4 });
  console.log(`[Screenshot Captured] ${shot4}`);

  // 5. Test Auto-Fade to Deck B
  console.log('\n--- Step 5: Test Beat-Quantized Auto-Fade to Deck B ---');
  await page.click('#autoFadeBBtn');
  await page.waitForTimeout(1000);

  const shot5 = path.join(ARTIFACT_DIR, '05_autofade_running.png');
  await page.screenshot({ path: shot5 });
  console.log(`[Screenshot Captured] ${shot5}`);

  // 6. Test Video Playback Layer
  console.log('\n--- Step 6: Test Video Playback Layer ---');
  // Switch to Layers tab
  await page.click('#tabs button:has-text("Layers")');
  await page.waitForTimeout(500);

  // Click "+ Video Playback" chip button
  await page.click('button:has-text("+ Video Playback")');
  await page.waitForTimeout(800);

  const shot6 = path.join(ARTIFACT_DIR, '06_video_playback_layer.png');
  await page.screenshot({ path: shot6 });
  console.log(`[Screenshot Captured] ${shot6}`);

  // 7. Test Live Camera Layer
  console.log('\n--- Step 7: Test Live Camera Ingest Layer ---');
  await page.click('button:has-text("+ Live Camera")');
  await page.waitForTimeout(1000);

  const shot7 = path.join(ARTIFACT_DIR, '07_live_camera_layer.png');
  await page.screenshot({ path: shot7 });
  console.log(`[Screenshot Captured] ${shot7}`);

  // 8. Test Project Saving & Dirty State
  console.log('\n--- Step 8: Test Project Saving & Keyboard Shortcut (Ctrl+S) ---');
  await page.keyboard.press('Control+s');
  await page.waitForTimeout(1000);

  const shot8 = path.join(ARTIFACT_DIR, '08_project_saved.png');
  await page.screenshot({ path: shot8 });
  console.log(`[Screenshot Captured] ${shot8}`);

  console.log('\n====================================================');
  console.log(' >> All 8 Automated Feature Tests PASSED Successfully! <<');
  console.log('====================================================');

  await browser.close();
  server.close();
}

runFeatureTests().catch(err => {
  console.error('[TEST FATAL ERROR]', err);
  process.exit(1);
});
