import { chromium } from 'playwright';
import { serveStatic } from './static-server.mjs';
import { mkdir, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import assert from 'node:assert/strict';
const [directory, output, ...extra] = process.argv.slice(2);
if (!directory || !output || extra.length) throw Error('Usage: node web-smoke.mjs STATIC_ROOT NEW_OUTPUT');
const root = path.resolve(directory);
const out = path.resolve(output);
await mkdir(out, { recursive:false });
const server = await serveStatic(root);
let browser;
try {
  browser = await chromium.launch({ executablePath:process.env.CHROMIUM, headless:true });
  const context = await browser.newContext({ viewport:{ width:1080, height:760 } });
  const origin = server.origin;
  const requests = [], errors = [];
  await context.route('**/*', route => {
    const url = route.request().url(); requests.push(url);
    return url.startsWith(origin + '/') ? route.continue() : route.abort();
  });
  const page = await context.newPage();
  page.on('pageerror', error => errors.push(String(error)));
  page.on('websocket', () => errors.push('Unexpected WebSocket'));
  const button = name => page.getByRole('button', { name, exact:true }).filter({ visible:true });
  const project = name => page.getByText(` / ${name}`, { exact:true });
  await page.goto(origin, { waitUntil:'networkidle' });
  await page.getByText('Project: alpha', { exact:true }).waitFor({ timeout:30000 });
  assert.equal(await page.locator('#error').innerText(), '');
  await context.setOffline(true);
  await button('Next project').click(); await project('beta').waitFor();
  await button('Next project').focus(); await page.keyboard.press('Enter'); await project('alpha').waitFor();
  await button('Reconnect').focus(); await page.keyboard.press('Tab');
  assert.equal(await page.evaluate(() => document.activeElement.getAttribute('aria-label')), 'Settings');
  await page.keyboard.press('Enter');
  await page.getByText('Theme collection', { exact:true }).waitFor();
  await button('Dark').click(); await page.screenshot({ path:path.join(out, 'nsh-web-dark.png') });
  await button('Light').click(); await page.screenshot({ path:path.join(out, 'nsh-web-light.png') });
  for (const name of ['Dashboard', 'Workspaces', 'Preferences', 'Handoff', 'Diagnostics']) {
    await button(name).click();
    assert.equal(await page.locator('#error').innerText(), '');
  }
  await button('Preferences').click();
  // The pinned renderer does not mirror Input.label into DOM aria-label.
  // Scope to the sole visible field on this page, then verify its actual result.
  const filter = page.locator('input:visible');
  assert.equal(await filter.count(), 1);
  await filter.fill('no-match');
  await page.getByText('Empty — no confirmed preferences match', { exact:true }).waitFor();
  await filter.fill('');
  await page.getByRole('button', { name:/^Details \d+$/ }).first().click();
  await page.getByText('Close detail', { exact:true }).waitFor();
  await button('Close detail').click();
  await button('Handoff').click();
  const task = page.locator('input[placeholder="Task ID"]:visible');
  assert.equal(await task.count(), 1);
  await task.fill('missing'); await button('Lookup').click();
  await page.getByText('Empty — no handoff for task missing in this project', { exact:true }).waitFor();
  await task.fill('x'.repeat(121)); await button('Lookup').click();
  await page.getByText(/Error — invalid task ID/).first().waitFor();
  await task.fill('shared'); await button('Reconnect').click();
  await button('Dashboard').click();
  await page.getByText('Project: alpha', { exact:true }).waitFor();
  await button('Connection target').click(); await button('Profile operator').click();
  assert.equal(await button('Inspect Profile').isDisabled(), true);
  assert.equal(await button('Enable writes…').isDisabled(), true);
  await button('Dashboard').click();
  await page.setViewportSize({ width:640, height:480 });
  await page.screenshot({ path:path.join(out, 'nsh-web-compact.png') });
  assert.equal(await page.locator('#error').innerText(), '');
  assert(!((await page.locator('body').innerText()).includes('nsh panicked')));
  assert.deepEqual(errors, []);
  assert(requests.every(url => url.startsWith(origin + '/')));
  const result = {
    status:'PASS', tested_git_sha:execFileSync('git', ['rev-parse','HEAD'], { encoding:'utf8' }).trim(),
    worktree_dirty:!!execFileSync('git', ['status','--porcelain'], { encoding:'utf8' }).trim(),
    backend:'actual NSH Rust WASM + browser DOM', browser:await browser.version(),
    offlineInteraction:true, fivePages:true, exactTaskValidation:true, preferenceFilterAndDetails:true,
    keyboardTabAndEnter:true, appearance:true, readOnlyOperator:true,
    resize:[1080,760,640,480], requests:requests.map(url => new URL(url).pathname), externalRequests:0,
    daemonConnection:'NOT_RUN: offline synthetic fixture, no native IPC or gateway'
  };
  await writeFile(path.join(out, 'web-result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result));
} finally { await browser?.close(); await server.close(); }
