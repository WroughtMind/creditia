// 逐帧渲染短片《脚注》（promo/short，three.js + WebGL2）
//   node promo/short/render.mjs stills 2.4,9,14 [out-dir] [scale]   抽几帧看看
//   node promo/short/render.mjs events [out.json]                    导出配乐要对齐的时间表
//   node promo/short/render.mjs video out.mp4 [fps] [from] [to] [scale]  整片（无声，CRF 12 母版）
// 竖版：ASPECT=9:16。需要 playwright（PLAYWRIGHT_DIR 指向带 node_modules/playwright 的目录）与 ffmpeg（FFMPEG）
// macOS 上走 ANGLE Metal（显卡）；别的系统默认 SwiftShader（软件渲染，慢），有显卡可以 GPU=1
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join, resolve, extname, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const require = createRequire(join(resolve(process.env.PLAYWRIGHT_DIR ?? join(here, '..')), 'noop.js'));
const { chromium } = require('playwright');
const FFMPEG = process.env.FFMPEG ?? 'ffmpeg';
const PORTRAIT = process.env.ASPECT === '9:16';

// 片子按模块加载，file:// 不让加载模块：起一个只读的本地静态服务；帧画面由页面直接 POST 回来
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.json': 'application/json', '.png': 'image/png', '.jpg': 'image/jpeg', '.ttf': 'font/ttf', '.woff2': 'font/woff2' };
let takeFrame = null;
const server = createServer((req, res) => {
  if (req.method === 'POST') {
    const chunks = [];
    req.on('data', (c) => chunks.push(c));
    req.on('end', () => { res.writeHead(204); res.end(); takeFrame?.(Buffer.concat(chunks)); takeFrame = null; });
    return;
  }
  const p = normalize(decodeURIComponent(new URL(req.url, 'http://x').pathname)).replace(/^([/\\])+/, '');
  const f = join(root, p);
  if (!f.startsWith(root) || !existsSync(f)) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'content-type': TYPES[extname(f)] ?? 'application/octet-stream' });
  res.end(readFileSync(f));
});
await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
const port = server.address().port;

const [mode = 'stills', arg, ...rest] = process.argv.slice(2);
const scale = Number(mode === 'video' ? rest[3] ?? 1 : mode === 'stills' ? rest[1] ?? 1 : 0.25);
const W = Math.round((PORTRAIT ? 1080 : 1920) * scale), H = Math.round((PORTRAIT ? 1920 : 1080) * scale);
const gpu = process.platform === 'darwin' ? ['--use-angle=metal', '--enable-gpu', '--ignore-gpu-blocklist']
  : process.env.GPU ? ['--enable-gpu', '--ignore-gpu-blocklist'] : ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'];
const browser = await chromium.launch({ headless: true, args: gpu });
const page = await browser.newPage({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });
page.on('pageerror', (e) => console.error('page:', e.message));
page.on('console', (m) => { if (m.type() === 'error' || m.type() === 'warning') console.error('console:', m.text()); });
await page.goto(`http://127.0.0.1:${port}/promo/short/index.html?scale=${scale}${PORTRAIT ? '&aspect=9:16' : ''}`);
for (let i = 0; ; i++) {
  const s = await page.evaluate(() => (window.ERROR ? 'error: ' + window.ERROR : window.READY ? 'ok' : ''));
  if (s) { if (s !== 'ok') throw new Error(s); break; }
  if (i > 1200) throw new Error('timeout');
  await new Promise((r) => setTimeout(r, 100));
}
console.error('renderer:', await page.evaluate(() => {
  const gl = document.querySelector('canvas').getContext('webgl2'), x = gl.getExtension('WEBGL_debug_renderer_info');
  return x ? gl.getParameter(x.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
}));

// 直接读 WebGL 画布的像素（RGBA，自下而上），交给 ffmpeg 翻转
async function frameRGBA(t) {
  const got = new Promise((ok) => { takeFrame = ok; });
  await page.evaluate(async (t) => {
    window.render(t);
    const gl = document.querySelector('canvas').getContext('webgl2');
    const w = gl.drawingBufferWidth, h = gl.drawingBufferHeight;
    if (!window.__px || window.__px.length !== w * h * 4) window.__px = new Uint8Array(w * h * 4);
    gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, window.__px);
    await fetch('/__frame', { method: 'POST', body: window.__px });
  }, t);
  return got;
}

function ffmpeg(args) { return spawn(FFMPEG, ['-hide_banner', '-loglevel', 'error', '-y', ...args], { stdio: ['pipe', 'inherit', 'inherit'] }); }
const RAW = ['-f', 'rawvideo', '-pix_fmt', 'rgba', '-s', `${W}x${H}`];

if (mode === 'stills') {
  const out = rest[0] ?? join(root, 'promo/.build/short/stills');
  mkdirSync(out, { recursive: true });
  for (const t of arg.split(',').map(Number)) {
    const t0 = Date.now();
    const buf = await frameRGBA(t);
    const f = join(out, `t${t.toFixed(2).padStart(5, '0')}.png`);
    const ff = ffmpeg([...RAW, '-i', '-', '-vf', 'vflip', '-frames:v', '1', f]);
    ff.stdin.end(buf); await new Promise((r) => ff.on('close', r));
    console.log(f, Date.now() - t0, 'ms');
  }
} else if (mode === 'events') {
  const ev = await page.evaluate(() => window.EVENTS);
  const out = arg ?? join(root, 'promo/.build/short/events.json');
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, JSON.stringify(ev));
  console.log('events →', out);
} else if (mode === 'video') {
  const fps = Number(rest[0] ?? 60), from = Number(rest[1] ?? 0);
  const to = Number(rest[2] ?? (await page.evaluate(() => window.EVENTS.duration)));
  const ff = ffmpeg([...RAW, '-framerate', String(fps), '-i', '-', '-vf', 'vflip', '-c:v', 'libx264', '-preset', 'medium', '-crf', '12', '-pix_fmt', 'yuv420p', arg]);
  const n = Math.round((to - from) * fps), t0 = Date.now();
  for (let i = 0; i < n; i++) {
    const buf = await frameRGBA(from + i / fps);
    if (!ff.stdin.write(buf)) await new Promise((r) => ff.stdin.once('drain', r));
    if (i % fps === 0) {
      const per = (Date.now() - t0) / (i + 1);
      process.stdout.write(`\r${(from + i / fps).toFixed(1)}s  ${per.toFixed(0)}ms/frame  eta ${((n - i) * per / 60000).toFixed(1)}min   `);
    }
  }
  ff.stdin.end();
  await new Promise((r) => ff.on('close', r));
  console.log('\nvideo →', arg);
}
await browser.close();
server.close();
