// 导出：PNG、SVG（文字转路径，任何地方打开都一样）、PDF、GIF（动画）、HTML 单文件（可交互、可离线）。
// Node 和 App 共用：栅格化由调用方注入（Node 用 resvg-js，App 用 resvg-wasm），压缩也注入。
import * as gifenc from 'gifenc';
// gifenc 在 Node 里是 CommonJS（导出挂在 default 上），在打包器里是 ESM
const G: any = (gifenc as any).GIFEncoder ? gifenc : (gifenc as any).default;
const { GIFEncoder, quantize, applyPalette } = G;
import { INTERACT_CSS, mountHits } from './interact.ts';
import type { Hit } from './svg.ts';
import { scrollTransforms, type ScrollMotion } from './scroll-motion.ts';

/** 一张可导出的作品 */
export interface Art { w: number; h: number; svg: string; hits: Hit[]; bg: string; title: string; motion?: Motion; crop?: Crop; /** 动画采样帧的持续时间（毫秒） */ frameDelay?: number }
/** 滚动片尾字幕的一帧：整卷 svg 里从 y 开始、高 h 的一段 */
export interface Crop { svg: string; w: number; h: number; y: number }
/** HTML 里的动态：scroll = 片尾字幕在银幕里滚动 */
export type Motion = ScrollMotion;

export interface Raster {
  png(svg: string, width?: number): Uint8Array;
  rgba(svg: string, width?: number): { w: number; h: number; data: Uint8Array };
  /** 文字转路径后的 SVG */
  flat(svg: string): string;
}
export type Deflate = (data: Uint8Array) => Uint8Array | Promise<Uint8Array>;

export const FORMATS = [
  { id: 'png', name: 'PNG', note: '社交媒体、聊天、文档' },
  { id: 'svg', name: 'SVG', note: 'README、网页，无损缩放' },
  { id: 'pdf', name: 'PDF', note: '打印、简历、作品集' },
  { id: 'gif', name: 'GIF', note: '会动：条形长出、字幕滚动' },
  { id: 'html', name: 'HTML', note: '单文件，可悬停查看细节，可离线' },
  { id: 'md', name: 'Markdown', note: 'CREDITS.md 区块' },
  { id: 'embed', name: '嵌入', note: '<creditia-card> 网页组件' },
] as const;

/** 常用画幅：作品居中放进画布 */
export const SIZES: { id: string; name: string; w: number; h: number }[] = [
  { id: 'native', name: '原尺寸', w: 0, h: 0 },
  { id: 'og', name: '链接预览 1200×630', w: 1200, h: 630 },
  { id: 'square', name: '方图 1080×1080', w: 1080, h: 1080 },
  { id: 'portrait', name: '竖图 1080×1350', w: 1080, h: 1350 },
  { id: 'story', name: '竖屏 1080×1920', w: 1080, h: 1920 },
  { id: 'a4', name: 'A4 2480×3508', w: 2480, h: 3508 },
];

const strip = (svg: string) => svg.replace(/^<svg[^>]*>/, '').replace(/<\/svg>\s*$/, '');

/** 把作品放进指定画布：等比缩放、居中、底色铺满 */
export function sized(a: Art, W: number, H: number, pad = 0.06): Art {
  if (!W || !H) return a;
  const k = Math.min((W * (1 - pad * 2)) / a.w, (H * (1 - pad * 2)) / a.h);
  const dx = (W - a.w * k) / 2, dy = (H - a.h * k) / 2;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}"><rect width="${W}" height="${H}" fill="${a.bg}"/><g transform="translate(${dx.toFixed(2)} ${dy.toFixed(2)}) scale(${k.toFixed(4)})">${strip(a.svg)}</g></svg>`;
  return { ...a, w: W, h: H, svg, hits: a.hits.map((x) => ({ ...x, x: dx + x.x * k, y: dy + x.y * k, w: x.w * k, h: x.h * k })) };
}

export function makeExporter(R: Raster, deflate?: Deflate) {
  const png = (a: Art, scale = 2) => R.png(a.svg, a.w * scale);
  const svg = (a: Art) => R.flat(a.svg).replace(/^<svg/, `<svg role="img" aria-label="${attr(a.title)}"`);

  async function pdf(a: Art, scale = 3): Promise<Uint8Array> {
    if (!deflate) throw new Error('PDF 需要压缩函数');
    const img = R.rgba(a.svg, a.w * scale);
    const n = img.w * img.h;
    const rgb = new Uint8Array(n * 3);
    const alpha = new Uint8Array(n);
    let opaque = true;
    for (let i = 0; i < n; i++) {
      rgb[i * 3] = img.data[i * 4]; rgb[i * 3 + 1] = img.data[i * 4 + 1]; rgb[i * 3 + 2] = img.data[i * 4 + 2];
      alpha[i] = img.data[i * 4 + 3];
      if (alpha[i] !== 255) opaque = false;
    }
    const pw = a.w * 0.75, ph = a.h * 0.75; // px → pt
    return buildPdf(a.title, pw, ph, img.w, img.h, await deflate(rgb), opaque ? null : await deflate(alpha));
  }

  /** frames：同一作品在动画各时刻的画面 */
  function gif(frames: Art[], o: { width?: number; fps?: number; hold?: number } = {}): Uint8Array {
    const enc = GIFEncoder();
    const delay = Math.round(1000 / (o.fps ?? 20));
    const width = o.width ?? Math.min(frames[0].w, 960);
    // 滚动长卷只栅格化一次；逐帧只裁取可见行。
    const c0 = frames[0].crop;
    if (c0 && frames.some(f => f.crop?.svg !== c0.svg)) throw new Error('Scroll frames must share one static reel');
    const whole = c0 ? R.rgba(c0.svg, (width * c0.w) / frames[0].w) : null;
    const cut = (f: Art, reel: ReturnType<Raster['rgba']>) => {
      const k = reel.w / f.crop!.w, h = Math.round(f.h * k);
      const top = Math.min(Math.round(f.crop!.y * k), reel.h - h);
      return { w: reel.w, h, data: reel.data.subarray(top * reel.w * 4, (top + h) * reel.w * 4) };
    };
    frames.forEach((f, i) => {
      const img = f.crop ? cut(f, whole!) : R.rgba(f.svg, width);
      const palette = quantize(img.data, 256, { format: 'rgb565' });
      const index = applyPalette(img.data, palette, 'rgb565');
      enc.writeFrame(index, img.w, img.h, { palette, delay: i === frames.length - 1 ? (o.hold ?? 2400) : o.fps === undefined ? f.frameDelay ?? delay : delay, repeat: 0 });
    });
    enc.finish();
    return enc.bytes();
  }

  /** 单文件 HTML：画面是文字转路径后的 SVG，交互层盖在上面。arts 多于一张时排成一页 */
  function html(arts: Art[], o: { title: string; lang?: string; bg?: string; note?: string } ): string {
    const bg = o.bg ?? arts[0]?.bg ?? '#111';
    const blocks = arts.map((a, i) => {
      const flat = svg(a);
      if (a.motion?.kind === 'scroll') {
        const m = a.motion;
        const keys = scrollTransforms(m.keys, a.h).map(key => `${key.offset * 100}%{transform:${key.transform};animation-timing-function:${key.easing ?? 'linear'}}`).join('');
        return `<style>@keyframes roll${i}{${keys}}</style><figure class="screen" style="--vw:${m.viewW}px;aspect-ratio:${m.viewW}/${m.viewH};--secs:${m.secs}s;--roll:roll${i}"><div class="reel"><div class="ct-art" id="a${i}" style="width:100%">${flat}</div></div><button class="replay" aria-label="replay">↻</button></figure>`;
      }
      return `<figure class="piece${arts.length > 1 && a.w / a.h > 1.7 ? ' wide' : ''}" style="max-width:${a.w}px"><div class="ct-art" id="a${i}">${flat}</div></figure>`;
    }).join('\n');
    const wall = arts.length > 1;
    const data = arts.map((a) => ({ w: a.w, h: a.h, hits: a.hits }));
    return `<!doctype html>
<html lang="${o.lang ?? 'zh'}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>${esc(o.title)}</title>
<meta name="generator" content="creditia">
<style>
*{box-sizing:border-box}html,body{margin:0}
body{min-height:100vh;background:${bg};display:flex;flex-direction:column;align-items:center;gap:5vmin;padding:6vmin 4vmin}
figure{margin:0;width:100%}
body.wall main{width:min(1640px,100%);columns:420px 3;column-gap:40px}
body.wall .piece{break-inside:avoid;margin:0 auto 40px;display:block}
body.wall .piece.wide{column-span:all;max-width:100%!important}
.piece{animation:rise .9s cubic-bezier(.2,.7,.2,1) both}
.piece .ct-art{width:100%;border-radius:6px;overflow:hidden}
.screen{position:relative;max-width:var(--vw);overflow:hidden;border-radius:4px;background:${bg};box-shadow:0 30px 80px rgba(0,0,0,.35)}
.screen .reel{position:absolute;inset:0}
.screen .reel .ct-art{position:absolute;left:0;top:0;animation:var(--roll) var(--secs) linear forwards}
.screen:hover .ct-art{animation-play-state:paused}
.replay{position:absolute;right:12px;bottom:12px;border:0;border-radius:99px;width:32px;height:32px;background:rgba(255,255,255,.14);color:#fff;font-size:16px;cursor:pointer;backdrop-filter:blur(8px)}
@media(prefers-reduced-motion:reduce){.screen{overflow-y:auto}.screen .reel,.screen .reel .ct-art{position:relative}.screen .reel .ct-art,.piece{animation:none}.replay{display:none}}
@keyframes rise{from{opacity:0;transform:translateY(16px) scale(.985);filter:blur(6px)}to{opacity:1;transform:none;filter:none}}
footer{font:12px/1.4 Inter,-apple-system,"PingFang SC",sans-serif;color:${lumOf(bg) > 0.5 ? 'rgba(0,0,0,.45)' : 'rgba(255,255,255,.45)'};text-align:center}
${INTERACT_CSS}
</style>
</head>
<body${wall ? ' class="wall"' : ''}>
${wall ? `<main>${blocks}</main>` : blocks}
<footer>${o.note ? esc(o.note) + ' · ' : ''}creditia</footer>
<script>
const DATA=${JSON.stringify(data)};
${mountHits.toString()}
DATA.forEach((d,i)=>{const el=document.getElementById('a'+i);if(el)mountHits(el,d.hits,d.w,d.h)});
document.querySelectorAll('.replay').forEach(b=>b.onclick=()=>{const r=b.parentElement.querySelector('.ct-art');r.style.animation='none';r.offsetHeight;r.style.animation=''});
</script>
</body>
</html>
`;
  }
  return { png, svg, pdf, gif, html, sized };
}

// ───────────────────────── PDF ─────────────────────────

const enc = new TextEncoder();
function buildPdf(title: string, pw: number, ph: number, iw: number, ih: number, rgbZ: Uint8Array, alphaZ: Uint8Array | null): Uint8Array {
  const parts: Uint8Array[] = [];
  const offsets: number[] = [];
  let size = 0;
  const push = (x: Uint8Array | string) => { const b = typeof x === 'string' ? enc.encode(x) : x; parts.push(b); size += b.length; };
  const obj = (n: number, body: string, stream?: Uint8Array) => {
    offsets[n] = size;
    push(`${n} 0 obj\n${body}\n`);
    if (stream) { push('stream\n'); push(stream); push('\nendstream\n'); }
    push('endobj\n');
  };
  push('%PDF-1.4\n%\xE2\xE3\xCF\xD3\n');
  const content = enc.encode(`q ${pw.toFixed(2)} 0 0 ${ph.toFixed(2)} 0 0 cm /Im0 Do Q`);
  obj(1, '<< /Type /Catalog /Pages 2 0 R >>');
  obj(2, '<< /Type /Pages /Kids [3 0 R] /Count 1 >>');
  obj(3, `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${pw.toFixed(2)} ${ph.toFixed(2)}] /Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>`);
  obj(4, `<< /Type /XObject /Subtype /Image /Width ${iw} /Height ${ih} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode${alphaZ ? ' /SMask 6 0 R' : ''} /Length ${rgbZ.length} >>`, rgbZ);
  obj(5, `<< /Length ${content.length} >>`, content);
  if (alphaZ) obj(6, `<< /Type /XObject /Subtype /Image /Width ${iw} /Height ${ih} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode /Length ${alphaZ.length} >>`, alphaZ);
  const info = alphaZ ? 7 : 6;
  obj(info, `<< /Title ${pdfStr(title)} /Producer (creditia) >>`);
  const xref = size;
  push(`xref\n0 ${info + 1}\n0000000000 65535 f \n`);
  for (let i = 1; i <= info; i++) push(`${String(offsets[i]).padStart(10, '0')} 00000 n \n`);
  push(`trailer\n<< /Size ${info + 1} /Root 1 0 R /Info ${info} 0 R >>\nstartxref\n${xref}\n%%EOF\n`);
  const out = new Uint8Array(size);
  let o = 0;
  for (const p of parts) { out.set(p, o); o += p.length; }
  return out;
}
/** PDF 字符串只放 ASCII；非 ASCII 标题转成 UTF-16BE 十六进制 */
function pdfStr(s: string): string {
  if (/^[\x20-\x7E]*$/.test(s)) return `(${s.replace(/[()\\]/g, '\\$&')})`;
  let hex = 'FEFF';
  for (const ch of s) { const c = ch.codePointAt(0)!; if (c > 0xffff) { const v = c - 0x10000; hex += ((0xd800 + (v >> 10)).toString(16) + (0xdc00 + (v & 0x3ff)).toString(16)).toUpperCase(); } else hex += c.toString(16).padStart(4, '0').toUpperCase(); }
  return `<${hex}>`;
}

const esc = (s: string) => String(s).replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]!);
const attr = (s: string) => esc(s).replace(/"/g, '&quot;');
function lumOf(hex: string) {
  if (!/^#[0-9a-f]{6}$/i.test(hex)) return 0;
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
