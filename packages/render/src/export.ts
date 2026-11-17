// 导出：PNG、SVG（文字转路径，任何地方打开都一样）、PDF、GIF（动画）、HTML 单文件（可交互、可离线）。
// Node 和 App 共用：栅格化由调用方注入（Node 用 resvg-js，App 用 resvg-wasm），压缩也注入。
import * as gifenc from 'gifenc';
// gifenc 在 Node 里是 CommonJS（导出挂在 default 上），在打包器里是 ESM
const G: any = (gifenc as any).GIFEncoder ? gifenc : (gifenc as any).default;
const { GIFEncoder, quantize, applyPalette } = G;
import { INTERACT_CSS, mountHits, placeTip } from './interact.ts';
import type { Hit } from './svg.ts';
import { scrollTransforms, scrollY, type ScrollMotion } from './scroll-motion.ts';
import { scrollPaperPose, scrollPaperTexture } from './scroll-paper.ts';
import { scrollAtlas, createScrollCompositor } from './scroll-compositor.ts';
import { prepareScrollCanvas } from './scroll-player.ts';
import { rasterExtent } from './raster-viewport.ts';

/** 一张可导出的作品 */
export interface Art { w: number; h: number; svg: string; hits: Hit[]; bg: string; title: string; selection?: { style: string; piece: string; lang: 'zh' | 'en'; variant?: string }; motion?: Motion; crop?: Crop; frameDelay?: number }
/** 滚动片尾字幕的一帧：整卷 svg 里从 y 开始、高 h 的一段 */
export interface Crop { svg: string; w: number; h: number; y: number; motion?: ScrollMotion; progress?: number; /** 调整画幅后，裁窗在输出画布内的位置 */ viewport?: { x: number; y: number; w: number; h: number } }
/** HTML 里的动态：scroll = 片尾字幕在银幕里滚动 */
export type Motion = ScrollMotion;

export interface Raster {
  png(svg: string, width?: number): Uint8Array;
  /** Straight (unpremultiplied) RGBA, matching decoded PNG bytes. */
  rgba(svg: string, width?: number): { w: number; h: number; data: Uint8Array };
  /** 文字转路径后的 SVG */
  flat(svg: string): string;
  /** Font files embedded as data URLs; hosts supply this for selectable SVG text. */
  embed?(svg: string): string;
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
  // Preserve the scrolling origin and visible window. Native raster adapters
  // can defer this named clip until after rendering the complete filtered reel.
  const content = a.crop
    ? `<defs><clipPath id="creditia-scroll-viewport"><rect width="${a.w}" height="${a.h}"/></clipPath></defs><g clip-path="url(#creditia-scroll-viewport)"><g transform="translate(0 ${-a.crop.y})">${strip(a.svg)}</g></g>`
    : strip(a.svg);
  const extent = rasterExtent(a.svg), actualK = Number(k.toFixed(4)), actualDx = Number(dx.toFixed(2)), actualDy = Number(dy.toFixed(2));
  const metadata = extent ? ` data-creditia-raster="${actualDy + extent[0] * actualK} ${actualDy + extent[1] * actualK}"${a.crop ? ` data-creditia-window="${actualDx} ${actualDy} ${a.w * actualK} ${a.h * actualK}" data-creditia-bg="${attr(a.bg)}"` : ''}` : '';
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}"${metadata}><rect width="${W}" height="${H}" fill="${a.bg}"/><g transform="translate(${dx.toFixed(2)} ${dy.toFixed(2)}) scale(${k.toFixed(4)})">${content}</g></svg>`;
  // Keep the source reel and map its viewport into the padded output canvas.
  // GIF can still rasterize one atlas while SVG/PNG retain native crop metadata.
  const viewport = a.crop?.viewport ?? { x: 0, y: 0, w: a.w, h: a.h };
  return { ...a, w: W, h: H, svg, ...(a.crop ? { crop: { ...a.crop, viewport: { x: dx + viewport.x * k, y: dy + viewport.y * k, w: viewport.w * k, h: viewport.h * k } } } : {}), hits: a.hits.map((x) => ({ ...x, x: dx + x.x * k, y: dy + x.y * k, w: x.w * k, h: x.h * k })) };
}

export function makeExporter(R: Raster, deflate?: Deflate) {
  const png = (a: Art, scale = 2) => R.png(a.svg, a.w * scale);
  const svg = (a: Art, fontMode: 'outline' | 'embed' = 'outline') => {
    if (fontMode === 'embed' && !R.embed) throw new Error('Font embedding is unavailable in this renderer');
    // Already outlined Satori art needs no parser/serializer round trip; it
    // would round existing path coordinates and can change edge pixels.
    const source = fontMode === 'embed' ? R.embed!(a.svg) : a.svg.includes('<text') ? R.flat(a.svg) : a.svg;
    return source.replace(/^<svg\b([^>]*?)(\/?)>/, (_root, attributes: string, slash: string) =>
      `<svg${attributes.replace(/\s+(?:role|aria-label)="[^"]*"/g, '')} role="img" aria-label="${attr(a.title)}"${slash}>`);
  };

  async function pdf(a: Art, scale = 3, pagePoints?: { w: number; h: number }): Promise<Uint8Array> {
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
    const pw = pagePoints?.w ?? a.w * 0.75, ph = pagePoints?.h ?? a.h * 0.75; // CSS px → pt; print presets use their paper dimensions
    return buildPdf(a.title, pw, ph, img.w, img.h, await deflate(rgb), opaque ? null : await deflate(alpha));
  }

  /** frames：同一作品在动画各时刻的画面 */
  function gif(frames: Art[], o: { width?: number; fps?: number; hold?: number; delays?: number[]; repeat?: number } = {}): Uint8Array {
    const enc = GIFEncoder();
    const delay = Math.round(1000 / (o.fps ?? 20));
    const width = o.width ?? Math.min(frames[0].w, 960);
    // 滚动长卷只栅格化一次；逐帧只裁取可见行。
    const c0 = frames[0].crop;
    // 公共导出器也接受不同长卷或普通帧的混合；只有同卷同倍率时复用栅格。
    const viewport = c0?.viewport;
    const motionSignature = JSON.stringify(c0?.motion);
    const sameReel = c0 && frames.every(f => f.crop?.svg === c0.svg && f.crop.w === c0.w && f.w === frames[0].w && f.h === frames[0].h && f.bg === frames[0].bg && JSON.stringify(f.crop.viewport) === JSON.stringify(viewport) && JSON.stringify(f.crop.motion) === motionSignature);
    const atlas = sameReel && c0.motion?.material ? scrollAtlas(c0.svg, c0.w, c0.h, c0.motion) : null;
    const logicalWidth = Math.round(width * (viewport?.w ?? c0?.w ?? frames[0].w) / frames[0].w);
    const whole = sameReel ? R.rgba(atlas?.svg ?? c0.svg, logicalWidth * (atlas ? 3 : 1)) : null;
    const composite = atlas && whole ? createScrollCompositor(whole, atlas, c0!.motion!) : null;
    const backgrounds = new Map<string, number[]>();
    const background = (color: string) => {
      let rgba = backgrounds.get(color);
      if (!rgba) {
        // 产品六位 RGB 无额外栅格；其他 CSS 颜色由原 SVG 引擎解析一次 1px 色样。
        rgba = /^#[0-9a-f]{6}$/i.test(color) ? [...[1, 3, 5].map(i => parseInt(color.slice(i, i + 2), 16)), 255]
          : Array.from(R.rgba(`<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1" fill="${attr(color)}"/></svg>`, 1).data.slice(0, 4));
        backgrounds.set(color, rgba);
      }
      return rgba;
    };
    const pad = (img: ReturnType<Raster['rgba']>, f: Art, view: Crop['viewport']) => {
      if (!view) return img;
      const scale = width / f.w, w = Math.round(width), height = Math.round(f.h * scale);
      const out = new Uint8Array(w * height * 4);
      const bg = background(f.bg);
      for (let i = 0; i < out.length; i += 4) { out[i] = bg[0]; out[i + 1] = bg[1]; out[i + 2] = bg[2]; out[i + 3] = bg[3]; }
      const x = Math.round(view.x * scale), y = Math.round(view.y * scale);
      for (let row = 0; row < img.h && row + y < height; row++) out.set(img.data.subarray(row * img.w * 4, (row + 1) * img.w * 4), ((row + y) * w + x) * 4);
      return { w, h: height, data: out };
    };
    const mixedMaterials = new Map<string, ReturnType<typeof createScrollCompositor>>();
    const mixedFrame = (f: Art) => {
      const c = f.crop;
      if (!c?.motion?.material) return R.rgba(f.svg, width);
      const w = Math.round(width * (c.viewport?.w ?? c.w) / f.w);
      const key = JSON.stringify([c.svg, c.motion, w]);
      let renderer = mixedMaterials.get(key);
      if (!renderer) {
        const a = scrollAtlas(c.svg, c.w, c.h, c.motion);
        renderer = createScrollCompositor(R.rgba(a.svg, w * 3), a, c.motion); mixedMaterials.set(key, renderer);
      }
      return pad(renderer.render(c.progress ?? 1, c.y) as ReturnType<Raster['rgba']>, f, c.viewport);
    };
    const cut = (f: Art, reel: ReturnType<Raster['rgba']>) => {
      const k = reel.w / f.crop!.w;
      const h = Math.round(viewport ? reel.w * viewport.h / viewport.w : f.h * k);
      const top = Math.max(0, Math.min(Math.round(f.crop!.y * k), reel.h - h));
      // gifenc 读取整块 ArrayBuffer，不能把带偏移/多余行的 subarray 传进去。
      const data = reel.data.slice(top * reel.w * 4, (top + h) * reel.w * 4);
      return pad({ w: reel.w, h, data }, f, viewport);
    };
    let elapsed = 0, encoded = 0;
    frames.forEach((f, i) => {
      const img = composite ? pad(composite.render(f.crop!.progress ?? 1, f.crop!.y) as ReturnType<Raster['rgba']>, f, viewport) : whole ? cut(f, whole) : mixedFrame(f);
      const transparent = img.data.some((value, index) => index % 4 === 3 && value < 128);
      const palette = quantize(img.data, transparent ? 255 : 256, { format: 'rgb565' });
      const index = applyPalette(img.data, palette, 'rgb565');
      if (transparent) {
        palette.unshift([0, 0, 0]);
        for (let p = 0; p < index.length; p++) index[p] = img.data[p * 4 + 3] < 128 ? 0 : index[p] + 1;
      }
      elapsed += o.delays?.[i] ?? (i === frames.length - 1 ? (o.hold ?? 2400) : o.fps === undefined ? f.frameDelay ?? delay : delay);
      // GIF 只存 10ms；量化累计边界，避免长卷逐帧舍入后偏离 App/HTML 时长。
      const until = Math.round(elapsed / 10) * 10;
      enc.writeFrame(index, img.w, img.h, { palette, transparent, transparentIndex: transparent ? 0 : -1, dispose: transparent ? 2 : 0, delay: until - encoded, repeat: o.repeat ?? 0 });
      encoded = until;
    });
    enc.finish();
    return enc.bytes();
  }

  /** 单文件 HTML：画面是文字转路径后的 SVG，交互层盖在上面。arts 多于一张时排成一页 */
  function html(arts: Art[], o: { title: string; lang?: string; bg?: string; note?: string; fontMode?: 'outline' | 'embed'; reducedMotion?: boolean; motionWidth?: number } ): string {
    const bg = o.bg ?? arts[0]?.bg ?? '#111';
    const materials: any[] = [];
    const blocks = arts.map((a, i) => {
      const flat = svg(a, o.fontMode);
      if (a.motion?.kind === 'scroll') {
        const m = a.motion;
        if (m.material) {
          const atlas = scrollAtlas(a.svg, a.w, a.h, m);
          // 字体只转路径；浏览器按实际可见宽度×DPR 一次栅格化图集，避免固定像素上限。
          const encoded = new TextEncoder().encode(R.flat(atlas.svg));
          let binary = '';
          for (let offset = 0; offset < encoded.length; offset += 0x8000) binary += String.fromCharCode(...encoded.subarray(offset, offset + 0x8000));
          materials.push({ i, width: o.motionWidth, source: `data:image/svg+xml;base64,${btoa(binary)}`, atlas: { originalW: atlas.originalW, originalH: atlas.originalH, layers: atlas.layers.map(({ paint: _paint, ...layer }) => ({ ...layer, paint: '' })) }, motion: { ...m, material: { style: m.material.style, layers: [] } } });
          return `<figure class="screen material-screen" data-material="${i}" style="--vw:${m.viewW}px;aspect-ratio:${m.viewW}/${m.viewH}"><div class="reel"><div class="ct-art" id="a${i}" style="width:100%">${flat}</div></div><button class="replay" aria-label="replay">↻</button></figure>`;
        }
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
.material-screen .reel .ct-art{animation:none}
.material-screen.material-ready .ct-art>svg{visibility:hidden}
.material-screen>canvas{position:absolute;inset:0;pointer-events:none}
.screen:hover .ct-art{animation-play-state:paused}
.replay{position:absolute;right:12px;bottom:12px;border:0;border-radius:99px;width:32px;height:32px;background:rgba(255,255,255,.14);color:#fff;font-size:16px;cursor:pointer;backdrop-filter:blur(8px)}
@media(prefers-reduced-motion:reduce){.screen{overflow-y:auto}.screen .reel,.screen .reel .ct-art{position:relative;transform:none!important}.screen .reel .ct-art,.piece{animation:none}.replay,.material-screen>canvas{display:none}.material-screen.material-ready .ct-art>svg{visibility:visible}}
@keyframes rise{from{opacity:0;transform:translateY(16px) scale(.985);filter:blur(6px)}to{opacity:1;transform:none;filter:none}}
footer{font:12px/1.4 Inter,-apple-system,"PingFang SC",sans-serif;color:${lumOf(bg) > 0.5 ? 'rgba(0,0,0,.45)' : 'rgba(255,255,255,.45)'};text-align:center}
${INTERACT_CSS}
${o.reducedMotion ? '.piece,.screen .reel{animation:none!important}.screen .reel .ct-art{top:0}.ct-hit,.ct-tip{transition:none!important}' : ''}
@media(prefers-reduced-motion:reduce){.piece,.screen .reel{animation:none!important}.screen .reel .ct-art{top:0}.ct-hit,.ct-tip{transition:none!important}}
</style>
</head>
<body${wall ? ' class="wall"' : ''}>
${wall ? `<main>${blocks}</main>` : blocks}
<footer>${o.note ? esc(o.note) + ' · ' : ''}creditia</footer>
<script>
const DATA=${JSON.stringify(data).replace(/</g, '\\u003c')};
const placeTip=(${placeTip.toString()});
const mountHits=(${mountHits.toString()});
DATA.forEach((d,i)=>{const el=document.getElementById('a'+i);if(el)mountHits(el,d.hits,d.w,d.h,placeTip)});
document.querySelectorAll('.replay').forEach(b=>b.onclick=()=>{const r=b.parentElement.querySelector('.ct-art');r.style.animation='none';r.offsetHeight;r.style.animation=''});
const MATERIALS=${JSON.stringify(materials).replace(/</g, '\\u003c')};
const sampleScrollPosition=(${scrollY.toString()});
const paperPose=(${scrollPaperPose.toString()});
const paperTexture=(${scrollPaperTexture.toString()});
const makeScrollCompositor=(${createScrollCompositor.toString()});
const prepareMaterialCanvas=(${prepareScrollCanvas.toString()});
MATERIALS.forEach(async config=>{
  const figure=document.querySelector('[data-material="'+config.i+'"]'), reel=figure.querySelector('.ct-art'), reduced=matchMedia('(prefers-reduced-motion: reduce)');
  let player=null, raf=0, time=0, origin=0, playing=false, loading=false;
  const paint=()=>{if(player)player.paint(time);reel.style.transform='translateY('+(-sampleScrollPosition(config.motion.keys,time)/config.atlas.originalH*100)+'%)';figure.dataset.progress=String(time)};
  const tick=now=>{if(!playing)return;time=Math.min(1,(now-origin)/(config.motion.secs*1000));paint();if(time<1)raf=requestAnimationFrame(tick);else playing=false;figure.dataset.playing=String(playing)};
  const play=value=>{cancelAnimationFrame(raf);playing=value&&!reduced.matches&&!!player;figure.dataset.playing=String(playing);if(playing){origin=performance.now()-time*config.motion.secs*1000;raf=requestAnimationFrame(tick)}};
  const ready=async()=>{if(player||loading||reduced.matches||${!!o.reducedMotion})return;loading=true;try{const bytes=Uint8Array.from(atob(config.source.split(',')[1]),c=>c.charCodeAt(0));player=await prepareMaterialCanvas(new Blob([bytes],{type:'image/svg+xml'}),config.atlas,config.motion,config.width??Math.round(figure.clientWidth*(devicePixelRatio||1)),undefined,makeScrollCompositor,sampleScrollPosition,paperPose,paperTexture);figure.__scrollMetrics=player.metrics;figure.insertBefore(player.canvas,figure.firstChild);figure.classList.add('material-ready');figure.dataset.state='ready';paint();play(true)}catch(error){figure.dataset.state='error';console.error(error)}finally{loading=false}};
  figure.querySelector('.replay').onclick=()=>{time=0;paint();play(true)};
  figure.addEventListener('pointerenter',()=>play(false));figure.addEventListener('pointerleave',()=>{if(time<1)play(true)});
  reduced.addEventListener('change',()=>{if(reduced.matches){play(false);reel.style.transform='none'}else{if(player){time=0;paint();play(true)}else ready()}});
  ready();
});
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
