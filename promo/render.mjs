// 逐帧渲染宣传片画面
//   node promo/render.mjs stills 1.5,9,12.5 [out-dir]     抽几帧看看
//   node promo/render.mjs events [out.json]               导出配乐要卡的时间点
//   node promo/render.mjs video out.mp4 [fps] [from] [to] 整片（无声）
// 需要 playwright（PLAYWRIGHT_DIR 指向带 node_modules/playwright 的目录）与 ffmpeg（FFMPEG）
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(join(resolve(process.env.PLAYWRIGHT_DIR ?? here), 'noop.js'));
const { chromium } = require('playwright');
const FFMPEG = process.env.FFMPEG ?? 'ffmpeg';

const [mode = 'stills', arg, ...rest] = process.argv.slice(2);
const browser = await chromium.launch({ args: ['--allow-file-access-from-files', '--disable-web-security'] });
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
page.on('pageerror', (e) => console.error('page:', e.message));
await page.goto(pathToFileURL(join(here, 'scene.html')).href);
for (let i = 0; i < 600; i++) {
  const s = await page.evaluate(() => (window.ERROR ? 'error: ' + window.ERROR : window.READY ? 'ok' : ''));
  if (s) { if (s !== 'ok') throw new Error(s); break; }
  await new Promise((r) => setTimeout(r, 100));
}
const canvas = await page.$('canvas');

async function frame(t, type = 'png') {
  await page.evaluate((t) => window.render(t), t);
  return canvas.screenshot({ type, quality: type === 'jpeg' ? 95 : undefined });
}

if (mode === 'stills') {
  const out = rest[0] ?? join(here, '.build', 'stills');
  mkdirSync(out, { recursive: true });
  for (const t of arg.split(',').map(Number)) writeFileSync(join(out, `t${t.toFixed(2)}.png`), await frame(t));
  console.log('stills →', out);
} else if (mode === 'events') {
  const ev = await page.evaluate(() => window.EVENTS);
  writeFileSync(arg ?? join(here, '.build', 'events.json'), JSON.stringify(ev));
  console.log('events ok');
} else if (mode === 'video') {
  const fps = Number(rest[0] ?? 30), from = Number(rest[1] ?? 0);
  const to = Number(rest[2] ?? (await page.evaluate(() => window.EVENTS.duration)));
  const ff = spawn(FFMPEG, ['-hide_banner', '-loglevel', 'error', '-y', '-f', 'image2pipe', '-framerate', String(fps), '-c:v', 'mjpeg', '-i', '-',
    '-c:v', 'libx264', '-preset', 'medium', '-crf', '16', '-pix_fmt', 'yuv420p', '-threads', '1', '-x264-params', 'rc-lookahead=20', arg], { stdio: ['pipe', 'inherit', 'inherit'] });
  const n = Math.round((to - from) * fps), t0 = Date.now();
  for (let i = 0; i < n; i++) {
    const buf = await frame(from + i / fps, 'jpeg');
    if (!ff.stdin.write(buf)) await new Promise((r) => ff.stdin.once('drain', r));
    if (i % fps === 0) process.stdout.write(`\r${(from + i / fps).toFixed(1)}s  ${((Date.now() - t0) / 1000 / (i + 1) * 1000).toFixed(0)}ms/frame   `);
  }
  ff.stdin.end();
  await new Promise((r) => ff.on('close', r));
  console.log('\nvideo →', arg);
}
await browser.close();
