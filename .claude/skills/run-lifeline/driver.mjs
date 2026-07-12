/* Lifeline app driver — the programmatic handle on the running app.
 *
 * The Antigravity engine (Rust/Axum) serves the web app at the root, so a full
 * end-to-end run is: build + launch the server, then drive the web UI in a
 * headless browser. This script is the browser half. It assumes the server is
 * already listening (see SKILL.md for the one-liner that starts it) and drives
 * the real product: onboarding → sign-up gate → portrait → coach → vault →
 * settings, taking a screenshot at each step and failing loudly on any console
 * error.
 *
 * Usage:
 *   node .claude/skills/run-lifeline/driver.mjs [baseUrl] [outDir]
 *   BASE defaults to http://127.0.0.1:8443, outDir to ./.driver-shots
 *
 * Playwright resolution: this container ships Playwright in the Node global
 * modules, so we import it by absolute path (bare "playwright" won't resolve
 * for an ESM file outside a package). Override with PLAYWRIGHT_MODULE if your
 * install lives elsewhere (`npm root -g` prints the dir).
 */

import { mkdirSync } from 'node:fs';

const PW = process.env.PLAYWRIGHT_MODULE || '/opt/node22/lib/node_modules/playwright/index.js';
const BASE = process.argv[2] || process.env.BASE || 'http://127.0.0.1:8443';
const OUT = process.argv[3] || '.driver-shots';
const CHROME = process.env.CHROMIUM_PATH || '/opt/pw-browsers/chromium';

mkdirSync(OUT, { recursive: true });
// Playwright is CommonJS: its named exports arrive under `.default` when
// imported into an ESM module, so fall back to that.
const pw = await import(PW);
const { chromium } = pw.chromium ? pw : pw.default;

const errors = [];
let failed = false;
const step = async (name, fn) => {
    try { await fn(); console.log('✓', name); }
    catch (e) { failed = true; console.log('✗', name, '—', String(e.message).split('\n')[0]); }
};

const browser = await chromium.launch({ executablePath: CHROME });
const ctx = await browser.newContext({ viewport: { width: 420, height: 900 } });
const page = await ctx.newPage();
page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
page.on('pageerror', (e) => errors.push('PAGEERROR: ' + e.message));
const shot = (n) => page.screenshot({ path: `${OUT}/${n}.png`, fullPage: true });

await step('load + skip onboarding', async () => {
    await page.goto(BASE, { waitUntil: 'networkidle' });
    await page.waitForTimeout(400);
    await page.locator('text=Skip intro').first().click().catch(() => {});
    await page.waitForTimeout(1600);
    await shot('01-gate');
});

await step('sign up (email + password)', async () => {
    await page.locator('.auth-seg button[data-mode="signup"]').click();
    await page.fill('#authEmail', `driver+${Date.now()}@lifeline.test`);
    await page.fill('#authPass', 'driverpass123');
    await page.locator('#authPrimary').click();
    await page.waitForTimeout(1600);
    if (!(await page.locator('.tabbar').count())) throw new Error('did not enter app');
    await shot('02-portrait');
});

await step('portrait shows a vitality score + Conductor banner', async () => {
    const n = await page.locator('.vitality-num .n').first().textContent();
    if (!n || Number.isNaN(parseInt(n, 10))) throw new Error('no vitality number');
    if (!(await page.locator('.conductor-banner').count())) throw new Error('no conductor banner');
});

await step('coach replies (proxy or on-device)', async () => {
    await page.locator('[data-nav="coach"]').first().click();
    await page.waitForTimeout(500);
    await page.fill('#coachInput', 'How is my sleep?');
    await page.locator('#coachSend').click();
    await page.waitForTimeout(1500);
    if (!(await page.locator('.msg.ai').count())) throw new Error('no coach reply');
    await shot('03-coach');
});

await step('vault stores encrypted journal', async () => {
    let sync = null;
    page.on('request', (r) => { if (r.url().includes('/sync/delta') && r.method() === 'POST') sync = r.postData(); });
    await page.locator('[data-nav="vault"]').first().click();
    await page.waitForTimeout(500);
    await page.locator('#newEntryBtn').click();
    await page.waitForTimeout(1400);
    if (sync) {
        const b = JSON.parse(sync);
        if (!b.encrypted_blob || !b.client_signature) throw new Error('sync payload not E2EE');
        if (b.encrypted_blob.includes('journal')) throw new Error('plaintext leaked in blob');
    }
    await shot('04-vault');
});

await step('settings shows account + on-device AI card', async () => {
    await page.locator('#moreTab').click();
    await page.waitForTimeout(300);
    await page.locator('.sheet-row[data-go="settings"]').click();
    await page.waitForTimeout(600);
    if (!(await page.locator('.ai-card').count())) throw new Error('no on-device AI card');
    await shot('05-settings');
});

await step('service worker registered (offline shell cached)', async () => {
    await page.waitForTimeout(500);
    const reg = await page.evaluate(async () => {
        if (!('serviceWorker' in navigator)) return false;
        const regs = await navigator.serviceWorker.getRegistrations();
        return regs.some((r) => !!r.active);
    });
    if (!reg) throw new Error('service worker not active');
});

console.log('\nconsole errors:', errors.length ? errors.join('\n  ') : 'none');
console.log('screenshots in:', OUT);
await browser.close();
if (failed || errors.length) process.exit(1);
console.log('\nALL STEPS PASSED');
