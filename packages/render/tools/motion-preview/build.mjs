import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fileURLToPath, pathToFileURL } from 'node:url';

const source = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(source, '../../../..');
const { build } = await import(pathToFileURL(root + '/apps/desktop/node_modules/vite/dist/node/index.js'));
const dir = root + '/output/playwright/.motion-preview-cache';
fs.mkdirSync(dir, { recursive: true });
const out = root + '/output/playwright/issue116-motion-preview.html';
const manifest = JSON.parse(fs.readFileSync(root + '/apps/desktop/src/fonts/manifest.json', 'utf8'));
function texts(folder) { return fs.readdirSync(folder, { withFileTypes: true }).flatMap(e => e.isDirectory() ? texts(path.join(folder, e.name)) : /\.(ts|json)$/.test(e.name) ? [fs.readFileSync(path.join(folder, e.name), 'utf8')] : []); }
const characters = [...new Set((texts(root + '/packages/render/src').join('') + fs.readFileSync(root + '/packages/render/fixtures/catnap.json', 'utf8')).split(''))].join('');
fs.writeFileSync(dir + '/characters.txt', characters);
fs.mkdirSync(dir + '/fonts', { recursive: true });
const fontHash = createHash('sha256').update('typographic-family-v1').update(characters).update(JSON.stringify(manifest));
for (const f of manifest) fontHash.update(fs.readFileSync(root + '/apps/desktop/src/fonts/' + f.file));
const signature = fontHash.digest('hex');
const cached = fs.existsSync(dir + '/font-signature') && fs.readFileSync(dir + '/font-signature', 'utf8') === signature;
async function subset(f) {
  const target = dir + '/fonts/' + f.file;
  if (!cached || !fs.existsSync(target)) await new Promise((resolve, reject) => {
    const child = spawn(process.env.PYFTSUBSET || 'pyftsubset', [root + '/apps/desktop/src/fonts/' + f.file, '--text-file=' + dir + '/characters.txt', '--output-file=' + target, '--layout-features=*', '--no-hinting', '--name-IDs=1,2,4,6,16,17', '--name-legacy', '--notdef-glyph', '--notdef-outline', '--recommended-glyphs'], { stdio: ['ignore', 'ignore', 'pipe'] });
    child.on('error', reject);
    let error = ''; child.stderr.on('data', s => error += s); child.on('close', code => code === 0 ? resolve() : reject(new Error(f.file + ': ' + error)));
  });
  return { ...f, url: 'data:font/ttf;base64,' + fs.readFileSync(target).toString('base64') };
}
const fonts = [];
for (let i = 0; i < manifest.length; i += 3) { fonts.push(...await Promise.all(manifest.slice(i, i + 3).map(subset))); console.log('fonts', fonts.length, '/', manifest.length); }
fs.writeFileSync(dir + '/font-signature', signature);
const config = entry => ({ root, configFile: false, define: { 'process.env.NODE_ENV': JSON.stringify('production') }, logLevel: 'error', publicDir: false, build: { write: false, minify: true, target: 'es2022', assetsInlineLimit: 1e9, lib: { entry, name: 'creditiaMotion', formats: ['iife'] } } });
const worker = await build(config(source + '/worker.ts'));
const workerCode = (Array.isArray(worker) ? worker[0] : worker).output.find(x => x.type === 'chunk').code;
const result = await build({ ...config(source + '/entry.ts'), plugins: [{ name: 'motion-preview-data', resolveId(id) { if (id.startsWith('motion-preview-')) return '\0' + id; }, load(id) { if (id === '\0motion-preview-worker') return 'export default ' + JSON.stringify(workerCode); if (id === '\0motion-preview-fonts') return 'export default ' + JSON.stringify(fonts); } }] });
const code = (Array.isArray(result) ? result[0] : result).output.find(x => x.type === 'chunk').code.replace(/<\/script/gi, '<\\/script');
const html = `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Creditia · 全部产物动效</title>
<style>
:root{font-family:-apple-system,BlinkMacSystemFont,"PingFang SC",sans-serif;color:#292820;background:#f5f4ef;color-scheme:light}*{box-sizing:border-box}body{margin:0}main{max-width:1480px;margin:auto;padding:24px 32px}header{display:flex;justify-content:space-between;gap:24px;align-items:flex-end;margin-bottom:20px}h1{font-size:25px;letter-spacing:-.04em;margin:0 0 8px;font-weight:600}.scope{color:#77746d;font-size:12px}.selectors{display:flex;gap:12px;align-items:flex-end}label{display:flex;flex-direction:column;gap:6px;font-size:11px;color:#77746d}select,button{font:inherit;font-size:13px;color:#292820;border:1px solid #c7c5bc;background:#fffefa;border-radius:6px;padding:9px 12px;cursor:pointer}select{min-width:135px;max-width:220px}button{min-width:65px}button:hover{background:#e9e7df}button:disabled{opacity:.45;cursor:default}.comparison{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:1px;background:#d8d6cc;border:1px solid #d8d6cc;border-radius:10px;overflow:hidden}.panel{background:#fffefa;padding:14px 18px;min-width:0}.label{font-size:12px;color:#77746d;margin-bottom:10px}.art{height:clamp(320px,calc(100dvh - 246px),800px);display:flex;align-items:center;justify-content:center;overflow:hidden}.art>svg{display:block;width:100%;height:100%;max-width:100%;max-height:100%;overflow:hidden}.scroll-screen{overflow:hidden;flex:none}.scroll-reel{width:100%;will-change:transform}.scroll-reel>svg{display:block;width:100%;height:auto}.transport{display:flex;gap:10px;align-items:center;margin-top:16px}.timeline{flex:1;min-width:70px;accent-color:#57574c}.progress{min-width:44px;text-align:right;font-size:12px;font-variant-numeric:tabular-nums;color:#77746d}.status{font-size:12px;min-height:17px;margin:10px 0;color:#77746d}.foot{font-size:11px;color:#848177;margin:8px 0}select:focus-visible,button:focus-visible,input:focus-visible{outline:2px solid #757565;outline-offset:3px}@media(max-width:850px){main{padding:20px 14px}header{display:block}.selectors{margin-top:18px;gap:8px;flex-wrap:wrap}.selectors label{flex:1}select{width:100%;max-width:none;min-width:105px}.art{height:clamp(280px,calc(100dvh - 300px),680px)}.panel{padding:12px 8px}}@media(max-width:560px){.comparison{grid-template-columns:1fr}.art{height:360px}}
</style><main><header><div><h1>全部产物动效</h1><div id="scope" class="scope"></div></div><div class="selectors"><label>风格<select id="style" aria-label="风格"></select></label><label>产物<select id="piece" aria-label="产物"></select></label><label>版式<select id="variant" aria-label="版式"></select></label></div></header><section class="comparison"><article class="panel"><div class="label">完整作品</div><div class="art" id="before"></div></article><article class="panel"><div class="label">动画</div><div class="art" id="after"></div></article></section><div class="transport"><button id="toggle" disabled>播放</button><button id="replay" disabled>重播</button><input id="timeline" class="timeline" type="range" min="0" max="1000" step="1" value="0" aria-label="播放进度" disabled><output id="progress" class="progress">0%</output></div><p id="status" class="status" role="status">正在载入…</p><p class="foot">catnap 项目样例 · 左侧保留完整构图，右侧可播放、暂停和逐帧查看</p></main><script>${code}</script></html>`;
if (fs.existsSync(out) && !fs.existsSync(dir + '/previous-preview.html')) fs.copyFileSync(out, dir + '/previous-preview.html');
fs.writeFileSync(out, html);
console.log(JSON.stringify({ output: out, fontFaces: fonts.length, bytes: Buffer.byteLength(html), sha256: createHash('sha256').update(html).digest('hex') }, null, 2));
