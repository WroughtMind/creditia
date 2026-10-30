import workerSource from 'motion-preview-worker';
import fonts from 'motion-preview-fonts';
import raw from '../../fixtures/catnap.json';
import { PIECES, variantName } from '../../src/registry.ts';
import { STYLES } from '../../src/styles/index.ts';
import { FORMS } from '../../src/classic/registry.ts';
import { variantsOf, defaultVariant } from '../../src/classic/art.ts';
import { ORDERED_THEMES, themeName, pieceName, variantName as classicVariant } from '../../src/classic/catalog.ts';
import { BRIDGED } from '../../src/classic/bridge.ts';

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
const workerUrl = URL.createObjectURL(new Blob([workerSource], { type: 'application/javascript' }));
function namespace(svg: string, prefix: string) {
  const ids = new Map([...svg.matchAll(/\bid="([^"]+)"/g)].map((m, i) => [m[1], prefix + i]));
  return svg.replace(/\bid="([^"]+)"/g, (_, id) => `id="${ids.get(id)}"`).replace(/url\(#([^)]*)\)/g, (_, id) => `url(#${ids.get(id) ?? id})`).replace(/((?:xlink:)?href=")#([^"]+)(")/g, (_, a, id, b) => a + '#' + (ids.get(id) ?? id) + b);
}
function paint(force = false) {
  if (!animation.length) return;
  const i = Math.min(animation.length - 1, Math.floor(time * (animation.length - 1)));
  if (force || i !== last) {
    stage.innerHTML = namespace(animation[i].svg, 'motion-'); last = i;
  }
  timeline.value = String(Math.round(time * 1000)); progress.value = `${Math.round(time * 100)}%`;
}
function tick(now: number) {
  if (!playing) return;
  time = Math.min(1, (now - origin) / duration); paint();
  if (time >= 1) play(false); else request = requestAnimationFrame(tick);
}
function play(value: boolean) {
  cancelAnimationFrame(request); playing = value && animation.length > 0;
  toggle.textContent = playing ? '暂停' : '播放'; toggle.setAttribute('aria-pressed', String(playing));
  if (playing) { if (time >= 1) time = 0; origin = performance.now() - time * duration; request = requestAnimationFrame(tick); }
}
function makeWorker() {
  const w = new Worker(workerUrl);
  w.postMessage({ init: true, raw, fonts: fonts.filter(f => !f.italic).map(f => ({ family: f.family, weight: f.weight, url: f.url })) });
  w.onmessage = async ({ data }) => {
    if (data.seq !== seq) return;
    if (data.error || data.empty) {
      pending = false; status.textContent = data.error ? `生成失败：${data.error}` : '这份样例缺少此产物所需的数据。';
      still.replaceChildren(); stage.replaceChildren(); document.documentElement.dataset.state = 'error'; return;
    }
    if (data.still) {
      stillArt = data.still; still.innerHTML = namespace(stillArt.svg, 'still-');
      stage.innerHTML = namespace(stillArt.svg, 'pending-'); return;
    }
    const job = seq;
    animation = data.animation;
    await document.fonts.ready;
    if (job !== seq) return;
    pending = false; time = 0; last = -1;
    duration = (animation[0]?.frameDelay ?? 65) * Math.max(1, animation.length - 1);
    status.textContent = piece.value === 'blame' ? '溯源使用明确标注的演示记录。' : '';
    toggle.disabled = replay.disabled = timeline.disabled = false;
    document.documentElement.dataset.state = 'ready'; paint(true);
    if (!matchMedia('(prefers-reduced-motion: reduce)').matches) play(true); else { time = 1; paint(); }
  };
  w.onerror = e => { pending = false; status.textContent = '生成失败：' + e.message; document.documentElement.dataset.state = 'error'; };
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
  play(false); seq++; animation = []; last = -1; time = 0;
  if (pending) { worker?.terminate(); worker = undefined; }
  worker ??= makeWorker(); pending = true;
  toggle.disabled = replay.disabled = timeline.disabled = true;
  status.textContent = '正在生成动画…'; document.documentElement.dataset.state = 'loading';
  const s = selectedStyle(); worker.postMessage({ seq, style: s.id, original: s.original, id: piece.value, variant: variant.value || undefined });
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
load();
