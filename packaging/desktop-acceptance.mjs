// Exercise the installed candidate through native WebDriver, without IPC mocks.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync, readFileSync, readdirSync, copyFileSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
import path from 'node:path';
const [application, output] = process.argv.slice(2);
assert(application && output, 'Usage: desktop-acceptance.mjs <installed-app> <evidence-dir>');
mkdirSync(output, { recursive: true });
const driver = spawn('tauri-driver', process.env.NATIVE_DRIVER ? ['--native-driver', process.env.NATIVE_DRIVER] : [], { stdio: ['ignore', 'pipe', 'pipe'] });
let log = '', session, appProcess;
driver.stdout.on('data', b => { log += b; });
driver.stderr.on('data', b => { log += b; });
const pause = ms => new Promise(r => setTimeout(r, ms));
async function request(method, route, body) {
  const response = await fetch(`http://127.0.0.1:4444${route}`, { method, headers: { 'Content-Type': 'application/json' }, body: body === undefined ? undefined : JSON.stringify(body), signal: AbortSignal.timeout(180000) });
  const result = await response.json();
  if (!response.ok || result.value?.error) throw Error(JSON.stringify(result));
  return result.value;
}
const command = (method, route, body) => request(method, `/session/${session}${route}`, body);
const js = (script, ...args) => command('POST', '/execute/sync', { script, args });
async function until(check, label, ms = 120000) {
  const deadline = Date.now() + ms;
  let last;
  while (Date.now() < deadline) {
    try { const value = await check(); if (value) return value; } catch (e) { last = e; }
    await pause(500);
  }
  throw Error(`Timed out: ${label}; ${last ?? ''}`);
}
async function element(selector) {
  const el = await command('POST', '/element', { using: 'css selector', value: selector });
  return el['element-6066-11e4-a52e-4f735466cecf'];
}
async function click(selector) {
  await until(async () => await js('return !!document.querySelector(arguments[0])', selector), selector);
  await command('POST', `/element/${await element(selector)}/click`, {});
}
async function ready() {
  await until(() => js(`return !document.body.classList.contains('busy') && [...document.querySelectorAll('#board .art img')].some(i => i.complete && i.naturalWidth > 0)`), 'rendered project artwork');
}
async function screenshot(name) {
  writeFileSync(path.join(output, name + '.png'), Buffer.from(await command('GET', '/screenshot'), 'base64'));
}
async function start() {
  let capabilities = { 'tauri:options': { application: path.resolve(application) } };
  if (process.platform === 'win32') {
    // Attach to the actual installed WebView2 app. Native driver launch is broken
    // with recent Evergreen runtimes (actions/runner-images#14738).
    appProcess = spawn(path.resolve(application), [], { env: { ...process.env,
      WEBVIEW2_USER_DATA_FOLDER: path.resolve(output, 'webview-profile'),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9222 --remote-debugging-address=127.0.0.1',
    }, stdio: ['ignore', 'pipe', 'pipe'] });
    appProcess.stdout.on('data', b => { log += b; });
    appProcess.stderr.on('data', b => { log += b; });
    await until(async () => (await fetch('http://127.0.0.1:9222/json/version')).ok, 'installed WebView2 debugging endpoint', 30000);
    capabilities = { browserName: 'MicrosoftEdge', 'ms:edgeOptions': { debuggerAddress: '127.0.0.1:9222' } };
  }
  const result = await request('POST', '/session', { capabilities: { alwaysMatch: capabilities } });
  session = result.sessionId;
  await command('POST', '/window/rect', { width: 1360, height: 880 });
  await ready();
}
const files = directory => {
  try { return readdirSync(directory); } catch (e) { if (e.code === 'ENOENT') return []; throw e; }
};
function stopApp() {
  if (appProcess) {
    spawnSync('taskkill', ['/PID', String(appProcess.pid), '/T', '/F']);
    appProcess = undefined;
  }
}
const records = [];
const record = (test, detail) => { records.push({ test, detail }); console.log(test, detail ?? ''); };
try {
  await until(() => request('GET', '/status'), 'native driver ready', 30000);
  await start();
  assert(await js('return !!window.__TAURI__?.core?.invoke'), 'Must run real native app');
  await screenshot('initial');
  const project = await js('return document.querySelector("#pname").textContent');
  assert(project && !/全部项目|All projects/.test(project));
  record('installed app renders real project', project);
  // Read native information only; all tested mutations go through UI clicks.
  const info = await command('POST', '/execute/async', { script: 'window.__TAURI__.core.invoke("app_info").then(arguments[arguments.length-1])', args: [] });
  writeFileSync(path.join(output, 'app-info.json'), JSON.stringify(info, null, 2));
  assert.equal(path.resolve(info.library), path.resolve(process.env.CREDITIA_HOME));
  await click('#lang'); await ready();
  if (await js('return document.documentElement.lang !== "en"')) { await click('#lang'); await ready(); }
  assert.equal(await js('return document.documentElement.lang'), 'en');
  record('switch language', 'English');
  await click('#project');
  assert.equal(await js('return document.querySelector("#re").disabled'), false);
  await click('#re');
  await until(() => js('return !document.querySelector("#toast").hidden && document.querySelector("#toast .msg")?.textContent.startsWith("Updated")'), 'real re-analysis completes');
  await ready();
  record('re-analyze through installed app');
  if (await js('return !document.querySelector("#group").hidden')) {
    await click('#group [data-g="model"]'); await ready();
    assert.equal(await js('return document.querySelector("#group [data-g=model]").getAttribute("aria-pressed")'), 'true');
    await click('#group [data-g="tool"]'); await ready();
    record('model and tool grouping');
  } else record('grouping not applicable', 'real checkout has no model attribution');
  const original = await js('return localStorage.getItem("creditia.theme")');
  await click('#dots [data-d="1"]'); await ready();
  const changed = await js('return localStorage.getItem("creditia.theme")');
  assert(changed && changed !== original);
  record('switch visual style', changed);
  await screenshot('changed-style');
  await click('#view [data-v="roam"]');
  await until(() => js('return !document.querySelector("#roam").hidden && !!document.querySelector("#roam canvas[data-ready=true], #roam img")'), 'roaming artwork');
  await screenshot('roam');
  await click('#view [data-v="wall"]'); await ready();
  record('wall and roam views');
  const tile = await element('#board .tile:not(.slot)');
  await command('POST', `/element/${tile}/value`, { text: '\uE007', value: ['\uE007'] });
  await until(() => js('return !document.querySelector("#focus").hidden && !!document.querySelector("#focus .xp")'), 'enlarged artwork');
  for (const format of ['png', 'svg', 'pdf', 'html', 'gif']) {
    await click('#focus .xp');
    if (!(await js('return !!document.querySelector(arguments[0])', `#focus .fmts [data-f="${format}"]`))) {
      assert.equal(format, 'gif');
      await click('#focus .xp');
      record('GIF not applicable to selected card'); continue;
    }
    const before = new Set(files(info.exportDir));
    await click(`#focus .fmts [data-f="${format}"]`);
    const file = await until(() => files(info.exportDir).find(f => !before.has(f) && f.endsWith('.' + format)), `${format} export`, 180000);
    await until(() => js('return document.querySelector("#toast .msg")?.textContent.startsWith("Saved to")'), 'native save completes');
    const data = readFileSync(path.join(info.exportDir, file));
    assert(data.length > 100);
    if (format === 'png') assert.equal(data.subarray(1, 4).toString(), 'PNG');
    if (format === 'pdf') assert.equal(data.subarray(0, 4).toString(), '%PDF');
    if (format === 'gif') assert.equal(data.subarray(0, 3).toString(), 'GIF');
    if (format === 'svg') assert.match(data.toString(), /<svg/);
    if (format === 'html') assert.match(data.toString(), /<svg/);
    copyFileSync(path.join(info.exportDir, file), path.join(output, file));
    record(`native ${format} export`, { file, bytes: data.length });
  }
  await screenshot('exports');
  await command('DELETE', ''); session = undefined; stopApp();
  await pause(2000);
  await start();
  assert.equal(await js('return document.documentElement.lang'), 'en');
  assert.equal(await js('return localStorage.getItem("creditia.theme")'), changed);
  assert.equal(await js('return document.querySelector("#pname").textContent'), project);
  await screenshot('restart');
  record('restart restores project, language, style and artwork');
  writeFileSync(path.join(output, 'result.json'), JSON.stringify({ passed: true, application, records }, null, 2));
} catch (error) {
  if (session) {
    await screenshot('failure').catch(() => {});
    const html = await command('GET', '/source').catch(() => '');
    writeFileSync(path.join(output, 'failure.html'), html);
  }
  writeFileSync(path.join(output, 'result.json'), JSON.stringify({ passed: false, error: String(error), records }, null, 2));
  throw error;
} finally {
  if (session) await command('DELETE', '').catch(() => {});
  stopApp();
  driver.kill();
  // Native WebDriver children may inherit these pipes after the session closes.
  driver.stdout.destroy(); driver.stderr.destroy(); driver.unref();
  writeFileSync(path.join(output, 'driver.log'), log);
}
