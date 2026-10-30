import workerSource from 'motion-preview-worker';
import fonts from 'motion-preview-fonts';
import raw from '../../fixtures/catnap.json';
import { PIECES, variantName } from '../../src/registry.ts';
import { STYLES } from '../../src/styles/index.ts';
import { FORMS } from '../../src/classic/registry.ts';
import { variantsOf, defaultVariant } from '../../src/classic/art.ts';
import { ORDERED_THEMES, themeName, pieceName, variantName as classicVariant } from '../../src/classic/catalog.ts';
import { BRIDGED } from '../../src/classic/bridge.ts';
import { scrollTransforms } from '../../src/scroll-motion.ts';

const byId = (id: string) => document.getElementById(id)!;
const style = byId('style') as HTMLSelectElement, piece = byId('piece') as HTMLSelectElement, variant = byId('variant') as HTMLSelectElement;
const still = byId('before'), stage = byId('after'), status = byId('status');
const timeline = byId('timeline') as HTMLInputElement, progress = byId('progress') as HTMLOutputElement;
const toggle = byId('toggle') as HTMLButtonElement, replay = byId('replay') as HTMLButtonElement;
const skip = new Set(['minimal', 'noir', 'glass', 'cinema', 'terminal']);
const styles = [...ORDERED_THEMES.map(theme => ({ id: theme.id, name: themeName(theme, 'zh'), original: true, theme })), ...STYLES.filter(s => !skip.has(s.id)).map(theme => ({ id: theme.id, name: theme.name, original: false, theme }))];
const classicIds = [...Object.keys(FORMS), ...BRIDGED, 'badge'];
const allIds = new Set([...classicIds, ...Object.keys(PIECES)]);
byId('scope').textContent = `${allIds.size} 类产物 · ${styles.length} 套风格 · 全部可选版式`;
const css = document.createElement('style');
css.textContent = fonts.map(f => `@font-face{font-family:"${f.family}";font-weight:${f.weight};font-style:${f.italic ? 'italic' : 'normal'};src:url(${f.url}) format('truetype');font-display:block}`).join('\n');
document.head.append(css);

let worker: Worker | undefined, seq = 0, pending = false, animation: any[] = [], stillArt: any;
let playing = false, time = 0, origin = 0, request = 0, duration = 1600, last = -1;
let scroll: Animation | undefined, resize: ResizeObserver | undefined;
let frameImages: HTMLImageElement[] = [], frameUrls: string[] = [];
const reduced = matchMedia('(prefers-reduced-motion: reduce)');
const workerUrl = URL.createObjectURL(new Blob([workerSource], { type: 'application/javascript' }));
function namespace(svg: string, prefix: string) {
  const ids = new Map([...svg.matchAll(/\bid="([^"]+)"/g)].map((m, i) => [m[1], prefix + i]));
  return svg.replace(/\bid="([^"]+)"/g, (_, id) => `id="${ids.get(id)}"`).replace(/url\(#([^)]*)\)/g, (_, id) => `url(#${ids.get(id) ?? id})`).replace(/((?:xlink:)?href=")#([^"]+)(")/g, (_, a, id, b) => a + '#' + (ids.get(id) ?? id) + b);
}
function resetScroll() {
  scroll?.cancel(); scroll = undefined; resize?.disconnect(); resize = undefined;
}
function resetFrameImages() {
  frameImages = [];
  for (const url of frameUrls) URL.revokeObjectURL(url);
  frameUrls = [];
}
async function prepareFrameImages(frames: any[], job: number) {
  const images = frames.map(frame => {
    const image = new Image(), url = URL.createObjectURL(new Blob([frame.svg], { type: 'image/svg+xml' }));
    frameUrls.push(url);
    image.alt = ''; image.draggable = false;
    image.style.cssText = 'display:block;width:100%;height:100%;object-fit:contain';
    image.src = url;
    return image;
  });
  // 与 App 一样，先解码完整 SVG（含内嵌 image），再复用同一图片元素播放。
  // 逐帧重建内联 SVG 会让图片解码落后于时间线，在负载下闪出空图。
  await Promise.all(images.map(image => image.decode()));
  if (job === seq) frameImages = images;
}
function showStill() {
  resetScroll(); resetFrameImages();
  if (stillArt) stage.innerHTML = namespace(stillArt.svg, 'motion-');
  time = 1; timeline.value = '1000'; progress.value = '100%';
}
function scrollScene() {
  const motion = stillArt.motion;
  const screen = document.createElement('div'), reel = document.createElement('div');
  screen.className = 'scroll-screen'; reel.className = 'scroll-reel';
  screen.style.background = stillArt.bg;
  reel.innerHTML = namespace(stillArt.svg, 'motion-'); screen.append(reel); stage.replaceChildren(screen);
  const fit = () => {
    const scale = Math.min(stage.clientWidth / motion.viewW, stage.clientHeight / motion.viewH);
    screen.style.width = `${motion.viewW * scale}px`; screen.style.height = `${motion.viewH * scale}px`;
  };
  fit(); resize = new ResizeObserver(fit); resize.observe(stage);
  duration = motion.secs * 1000;
  scroll = reel.animate(scrollTransforms(motion.keys, stillArt.h), { duration, easing: 'linear', fill: 'both' });
  scroll.pause(); scroll.currentTime = 0;
}
function paint(force = false) {
  if (scroll) scroll.currentTime = time * duration;
  else if (animation.length) {
    const i = Math.min(animation.length - 1, Math.floor(time * (animation.length - 1)));
    if (force || i !== last) {
      if (frameImages.length) stage.replaceChildren(frameImages[i]);
      else stage.innerHTML = namespace(animation[i].svg, 'motion-');
      last = i;
    }
  }
  timeline.value = String(Math.round(time * 1000)); progress.value = `${Math.round(time * 100)}%`;
}
function tick(now: number) {
  if (!playing) return;
  time = Math.min(1, (now - origin) / duration); paint();
  if (time >= 1) play(false); else request = requestAnimationFrame(tick);
}
function play(value: boolean) {
  cancelAnimationFrame(request); playing = value && !reduced.matches && (!!scroll || animation.length > 0);
  toggle.textContent = playing ? '暂停' : '播放'; toggle.setAttribute('aria-pressed', String(playing));
  if (playing) { if (time >= 1) time = 0; paint(); origin = performance.now() - time * duration; request = requestAnimationFrame(tick); }
}
function fail(message: string) {
  pending = false; play(false); animation = []; showStill();
  worker?.terminate(); worker = undefined;
  toggle.disabled = replay.disabled = timeline.disabled = true;
  status.textContent = message; document.documentElement.dataset.state = 'error';
}
function makeWorker() {
  const w = new Worker(workerUrl);
  w.postMessage({ init: true, raw, fonts: fonts.filter(f => !f.italic).map(f => ({ family: f.family, weight: f.weight, url: f.url })) });
  w.onmessage = async ({ data }) => {
    if (w !== worker || data.seq !== seq) return;
    if (data.error || data.empty) { fail(data.error ? `生成失败：${data.error}` : '这份样例缺少此产物所需的数据。'); return; }
    if (data.still) {
      stillArt = data.still; still.innerHTML = namespace(stillArt.svg, 'still-');
      stage.innerHTML = namespace(stillArt.svg, 'pending-');
      if (!data.complete) return;
    }
    const job = seq;
    animation = data.animation ?? [];
    await document.fonts.ready;
    if (w !== worker || job !== seq) return;
    // Satori 已将字转为路径；这些独立图片无需继承页面字体。仍含文字的作品保持内联。
    if (!reduced.matches && animation.length && animation.every(frame => !/<text\b/.test(frame.svg))) {
      try { await prepareFrameImages(animation, job); }
      catch (error) {
        if (w === worker && job === seq) fail(`图片准备失败：${error instanceof Error ? error.message : String(error)}`);
        return;
      }
      if (w !== worker || job !== seq) return;
    }
    pending = false; time = 0; last = -1;
    status.textContent = piece.value === 'blame' ? '溯源使用明确标注的演示记录。' : '';
    document.documentElement.dataset.state = 'ready';
    if (reduced.matches) {
      showStill(); status.textContent = '已按系统设置减少动态效果。';
      document.documentElement.dataset.animationKind = 'static';
      return;
    }
    if (stillArt.motion?.kind === 'scroll') scrollScene();
    else {
      if (animation.length < 2) { fail('这份作品没有可播放的动画帧。'); return; }
      duration = (animation[0]?.frameDelay ?? 65) * (animation.length - 1);
    }
    document.documentElement.dataset.animationKind = scroll ? 'scroll' : 'frames';
    toggle.disabled = replay.disabled = timeline.disabled = false;
    paint(true); play(true);
  };
  w.onerror = e => { if (w === worker) fail('生成失败：' + e.message); };
  return w;
}
const selectedStyle = () => styles[Number(style.value)];
function options(select: HTMLSelectElement, values: { value: string; name: string }[], preferred?: string) {
  select.replaceChildren(...values.map(v => { const option = document.createElement('option'); option.value = v.value; option.textContent = v.name; return option; }));
  if (preferred && values.some(v => v.value === preferred)) select.value = preferred;
}
function variants(preferred?: string) {
  const s = selectedStyle(), id = piece.value;
  const vs = s.original ? variantsOf(id) : PIECES[id]?.variants ?? [];
  const def = s.original ? defaultVariant(id, s.theme as any) : (s.theme as any).prefer[id];
  options(variant, (vs.length ? vs : ['']).map(v => ({ value: v, name: v ? (s.original ? classicVariant(v, 'zh') : variantName(v, 'zh')) : '默认' })), preferred ?? def);
  variant.disabled = vs.length < 2;
}
function pieces(preferred?: string) {
  const s = selectedStyle();
  options(piece, (s.original ? classicIds : Object.keys(PIECES)).map(id => ({ value: id, name: s.original ? pieceName(id, 'zh', raw) : PIECES[id].name })), preferred);
  variants();
}
function load() {
  play(false); resetScroll(); resetFrameImages(); seq++; animation = []; stillArt = undefined; last = -1; time = 0;
  still.replaceChildren(); stage.replaceChildren(); timeline.value = '0'; progress.value = '0%';
  if (pending) { worker?.terminate(); worker = undefined; }
  worker ??= makeWorker(); pending = true;
  toggle.disabled = replay.disabled = timeline.disabled = true;
  status.textContent = '正在生成动画…'; document.documentElement.dataset.state = 'loading';
  const s = selectedStyle(); worker.postMessage({ seq, style: s.id, original: s.original, id: piece.value, variant: variant.value || undefined, staticOnly: reduced.matches });
  const params = new URLSearchParams({ style: s.id, piece: piece.value, variant: variant.value }); history.replaceState(null, '', '#' + params);
}
options(style, styles.map((s, i) => ({ value: String(i), name: s.name })));
const initial = new URLSearchParams(location.hash.slice(1));
const start = styles.findIndex(s => s.id === (initial.get('style') || 'ink')); style.value = String(Math.max(0, start));
pieces(initial.get('piece') || 'asterisk'); if (initial.has('variant')) variants(initial.get('variant')!);
style.addEventListener('change', () => { const p = piece.value; pieces(p); load(); });
piece.addEventListener('change', () => { variants(); load(); });
variant.addEventListener('change', load);
toggle.addEventListener('click', () => play(!playing));
replay.addEventListener('click', () => { time = 0; paint(true); play(true); });
timeline.addEventListener('input', () => { play(false); time = Number(timeline.value) / 1000; paint(); });
document.addEventListener('visibilitychange', () => { if (document.hidden) play(false); });
reduced.addEventListener('change', () => {
  if (!reduced.matches || !stillArt) { load(); return; }
  play(false); seq++; animation = []; showStill();
  if (pending) { worker?.terminate(); worker = undefined; }
  pending = false; toggle.disabled = replay.disabled = timeline.disabled = true;
  status.textContent = '已按系统设置减少动态效果。';
  document.documentElement.dataset.state = 'ready'; document.documentElement.dataset.animationKind = 'static';
});
window.addEventListener('pagehide', () => play(false));
load();
