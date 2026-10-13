// creditia App：一面展墙。顶部切换项目，左下角圆点切换风格，所有作品直接摆出来；
// 悬停看细节、换版式、选格式导出；拖到右下角的导出坞也能导出。
import { buildModel, type Model } from '../../../packages/render/src/classic/data.ts';
import { creditsBlock, mergeCreditsMd } from '../../../packages/render/src/classic/text.ts';
import { FORMATS, SIZES, type Art } from '../../../packages/render/src/export.ts';
import { INTERACT_CSS, mountHits } from '../../../packages/render/src/interact.ts';
import { KITS, CLASSIC_COUNT, kitById, bootKits, type Kit } from './kits.ts';
import { exportArt, type Format } from './engine.ts';
import { LOGO_SVG } from './logo.ts';

// ───────────────────────── 桥：Tauri 或浏览器调试 ─────────────────────────
const native = (window as any).__TAURI__?.core?.invoke as ((cmd: string, args?: any) => Promise<any>) | undefined;
/** 官网上的演示：页面先放好样例档案（window.__CREDITIA_DEMO__），导出的文件交给外层页面摆到「桌面」上 */
const demo = (window as any).__CREDITIA_DEMO__ as { projects: any[]; credits: Record<string, any>; doctor: any[] } | undefined;
async function api(cmd: string, args: any = {}): Promise<any> {
  if (native) return native(cmd, args);
  const get = (u: string) => fetch(u).then((r) => { if (!r.ok) throw new Error(r.statusText); return r.json(); });
  switch (cmd) {
    case 'projects': return demo ? demo.projects : get('/__dev/projects');
    case 'credits': if (demo) { const c = demo.credits[args.projectId]; if (!c) throw new Error('not found'); return c; } return get(`/__dev/credits?id=${encodeURIComponent(args.projectId)}`);
    case 'config_of': return {};
    case 'doctor': return demo ? demo.doctor : get('/__dev/doctor');
    case 'save_file': {
      if (demo && parent !== window) { parent.postMessage({ type: 'creditia:saved', name: args.name, mime: args.mime, bytes: args.bytes }, location.origin); return `~/Desktop/${args.name}`; }
      const a = document.createElement('a');
      a.href = URL.createObjectURL(new Blob([b64bytes(args.bytes) as BlobPart], { type: args.mime ?? 'application/octet-stream' }));
      a.download = args.name; a.click(); setTimeout(() => URL.revokeObjectURL(a.href), 2000);
      return `~/Downloads/${args.name}`;
    }
    case 'set_display': return null;
    default: throw new Error(L('这一步要在 App 里运行', 'Only available in the desktop app'));
  }
}
const b64 = (bytes: Uint8Array) => { let s = ''; for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000)); return btoa(s); };
const b64bytes = (s: string) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));

// ───────────────────────── 状态 ─────────────────────────
const store = { get: (k: string) => { try { return localStorage.getItem('creditia.' + k); } catch { return null; } }, set: (k: string, v: string) => { try { localStorage.setItem('creditia.' + k, v); } catch { /* 没有存储也能用 */ } } };
const state = {
  projects: [] as any[], current: null as any, raw: null as any, model: null as Model | null,
  style: kitById(store.get('theme') ?? '') ?? KITS[0],
  lang: (store.get('lang') as 'zh' | 'en') ?? 'zh',
  /** 切项目的序号：慢的旧请求回来时如果已经切走，就丢弃 */
  openSeq: 0,
  variants: {} as Record<string, string>,
  size: store.get('size') ?? 'native',
  arts: new Map<string, Art>(),
  token: 0,
};
const L = (zh: string, en: string) => (state.lang === 'zh' ? zh : en);
const $ = <T extends HTMLElement = HTMLElement>(s: string) => document.querySelector(s) as T;
const fmt = (n: number) => n.toLocaleString('en-US');
const esc = (s: unknown) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!);
function toast(msg: string, ms = 2600) { const el = $('#toast'); el.textContent = msg; el.hidden = false; clearTimeout((toast as any)._t); (toast as any)._t = setTimeout(() => (el.hidden = true), ms); }

const tiles = () => state.style.board;
const keyOf = (b: { id: string; variant?: string }, i: number) => `${b.id}#${i}`;

// ───────────────────────── 启动 ─────────────────────────
async function boot() {
  bootKits();
  const st = document.createElement('style'); st.textContent = INTERACT_CSS; document.head.appendChild(st);
  $('#logo').innerHTML = LOGO_SVG;
  renderDots(); renderDock(); wireTop();
  try { state.projects = await api('projects'); } catch (e: any) { return empty(L('读不到作品库', 'Cannot read the library'), String(e?.message ?? e)); }
  if (!state.projects.length) return empty(L('还没有分析过的项目', 'No analyzed projects yet'), '', 'creditia analyze');
  const last = state.projects.find((p) => p.id === store.get('project')) ?? state.projects[0];
  await openProject(last);
  addEventListener('resize', () => layout());
}
function empty(title: string, detail = '', cmd = '') {
  $('#board').innerHTML = ''; document.body.classList.remove('busy');
  const el = $('#empty'); el.hidden = false;
  el.innerHTML = `<div><b>${esc(title)}</b>${esc(detail)}${cmd ? `<br><span>${L('在项目目录里运行', 'Run in your project')}</span><br><code>${esc(cmd)}</code>` : ''}</div>`;
  $('#pname').textContent = 'creditia'; $('#pmeta').textContent = '';
}

async function openProject(p: any) {
  const seq = ++state.openSeq;
  state.current = p; store.set('project', p.id);
  state.raw = null; state.model = null; state.arts.clear(); state.variants = {};
  ++state.token; // 让正在画的旧项目停下
  $('#board').innerHTML = '';
  $('#pname').textContent = p.name; $('#pmeta').textContent = L('读取中…', 'Loading…');
  let raw: any;
  try { raw = await api('credits', { projectId: p.id }); }
  catch {
    if (seq !== state.openSeq) return;
    return empty(L('这个项目还没有档案', 'No credits for this project yet'), '', `cd ${p.path} && creditia analyze`);
  }
  if (seq !== state.openSeq) return; // 已经切到别的项目
  state.raw = raw;
  $('#empty').hidden = true;
  rebuildModel();
  await paint();
}
function rebuildModel() {
  state.model = buildModel(state.raw, { director: false, lang: state.lang });
  const m = state.model;
  $('#pname').textContent = m.project || state.current?.name;
  $('#pmeta').textContent = `${fmt(m.commits)} ${L('次提交', 'commits')} · ${fmt(m.linesAdded)} ${L('行', 'lines')}`;
}

// ───────────────────────── 展墙 ─────────────────────────
const curVariant = (id: string, k: string, b: { variant?: string }) => state.style.current(id, state.variants[k] ?? b.variant);
const drawArt = (id: string, variant?: string) => state.style.render(state.raw, state.lang, id, variant);

async function paint() {
  const m = state.model; if (!m) return;
  const st = state.style;
  const token = ++state.token;
  const board = $('#board');
  // 墙上已经有作品（换风格、换语言）：旧的先留着，新的在后台全部画好，再一次原地换上，整面墙只滑动一次；
  // 一张一张换的话，每换一张后面的都要重排，墙会连着抽动十几次。空墙就画一张摆一张。
  const old = [...board.querySelectorAll('.tile')] as HTMLElement[];
  const batch = old.length > 0;
  document.body.classList.toggle('busy', batch);
  const fresh: HTMLElement[] = [];
  const keep = new Set<string>();
  for (const [i, b] of tiles().entries()) {
    const k = keyOf(b, i);
    const variant = curVariant(b.id, k, b);
    let a: Art | null = null;
    try { a = await drawArt(b.id, variant); } catch (e) { console.error(b.id, e); }
    if (token !== state.token) return;
    if (!a) continue;
    state.arts.set(k, a); keep.add(k);
    const tile = document.createElement('div');
    tile.className = 'tile';
    tile.dataset.k = k;
    tile.dataset.w = String(a.w); tile.dataset.h = String(a.h);
    tile.dataset.span = String(b.span ?? (a.w / a.h > 2.2 ? 2 : 1));
    const vs = st.variants(b.id);
    tile.innerHTML = tileHtml(b.id, vs, variant);
    const artEl = tile.querySelector('.art') as HTMLElement;
    artEl.innerHTML = a.svg;
    mountHits(artEl, a.hits, a.w, a.h);
    wireTile(tile, b.id, k, vs, variant ?? '');
    if (batch) { fresh.push(tile); continue; }
    board.appendChild(tile);
    layout();
  }
  for (const tile of fresh) {
    const prev = old.shift();
    if (prev) {
      // 接着旧的那张的位置和宽度，淡入，再滑到自己的位置
      tile.style.left = prev.style.left; tile.style.top = prev.style.top; tile.style.width = prev.style.width;
      tile.classList.add('swap', 'placed');
      prev.replaceWith(tile);
    } else board.appendChild(tile);
  }
  old.forEach((t) => t.remove());
  document.body.classList.remove('busy');
  for (const k of [...state.arts.keys()]) if (!keep.has(k)) state.arts.delete(k);
  layout();
}
function tileHtml(id: string, vs: string[], cur?: string) {
  return `<div class="name">${esc(state.style.pieceName(id, state.lang))}<span class="how">${L('点击放大 · 拖动导出', 'Click to enlarge · drag to export')}</span></div>
      <div class="bar glass">${vs.length > 1 ? `<button data-act="prev" title="${L('上一种版式', 'Previous layout')}">‹</button><span class="vname">${esc(state.style.variantName(cur ?? '', state.lang))}</span><button data-act="next" title="${L('下一种版式', 'Next layout')}">›</button>` : ''}<button data-act="export" title="${L('导出', 'Export')}">${L('导出', 'Export')} ↓</button></div>
      <div class="art"></div>`;
}

/** 瀑布流：一栏宽度跟着窗口变，横向作品跨两栏，小件不放大 */
function layout() {
  const board = $('#board');
  const tiles = [...board.querySelectorAll('.tile')] as HTMLElement[];
  const W = Math.min(innerWidth - 120, 1680);
  const gap = 44;
  const cols = Math.max(1, Math.min(5, Math.floor((W + gap) / (360 + gap))));
  const cw = (W - gap * (cols - 1)) / cols;
  const hs = new Array(cols).fill(0);
  for (const t of tiles) {
    const a = { w: Number(t.dataset.w), h: Number(t.dataset.h) };
    let span = Math.min(cols, Number(t.dataset.span) || 1);
    const full = cw * span + gap * (span - 1);
    const w = Math.min(full, a.w * (a.w < 500 ? 1.15 : 1));
    const h = (w / a.w) * a.h;
    // 找能放下的最矮位置
    let best = 0, bestY = Infinity;
    for (let c = 0; c + span <= cols; c++) { const y = Math.max(...hs.slice(c, c + span)); if (y < bestY - 1) { bestY = y; best = c; } }
    const x = best * (cw + gap) + (full - w) / 2;
    t.style.left = `${x}px`; t.style.top = `${bestY}px`; t.style.width = `${w}px`;
    for (let c = best; c < best + span; c++) hs[c] = bestY + h + gap + 18;
    if (!t.classList.contains('placed')) requestAnimationFrame(() => t.classList.add('placed')); // 第一次放好之后，再挪位置才有过渡
  }
  board.style.width = `${W}px`;
  board.style.height = `${Math.max(...hs)}px`;
}

function wireTile(tile: HTMLElement, id: string, k: string, vs: string[], cur: string) {
  tile.querySelector('.bar')!.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest('button'); if (!b) return;
    e.stopPropagation();
    const act = b.dataset.act;
    if (act === 'prev' || act === 'next') {
      const i = vs.indexOf(cur);
      state.variants[k] = vs[(i + (act === 'next' ? 1 : vs.length - 1)) % vs.length];
      return repaintTile(tile, id, k);
    }
    if (act === 'export') toggleFormats(tile, id, k);
  });
  const artEl = tile.querySelector('.art') as HTMLElement;
  artEl.addEventListener('pointerdown', (e) => startDrag(e, tile, id, k));
}
/** 只重画一张（换版式） */
async function repaintTile(tile: HTMLElement, id: string, k: string) {
  const i = Number(k.split('#')[1]);
  const b = tiles()[i];
  const variant = curVariant(id, k, b);
  const token = state.token;
  const a = await drawArt(id, variant);
  if (!a || token !== state.token) return;
  state.arts.set(k, a);
  const vs = state.style.variants(id);
  const fresh = document.createElement('div');
  fresh.className = 'tile placed'; fresh.dataset.k = k; fresh.dataset.w = String(a.w); fresh.dataset.h = String(a.h); fresh.style.left = tile.style.left; fresh.style.top = tile.style.top; fresh.style.width = tile.style.width; fresh.dataset.span = String(b.span ?? (a.w / a.h > 2.2 ? 2 : 1)); fresh.style.animation = 'none';
  fresh.innerHTML = tileHtml(id, vs, variant);
  const artEl = fresh.querySelector('.art') as HTMLElement;
  artEl.innerHTML = a.svg;
  mountHits(artEl, a.hits, a.w, a.h);
  wireTile(fresh, id, k, vs, variant ?? '');
  tile.replaceWith(fresh);
  layout();
}

// ───────────────────────── 导出 ─────────────────────────
const closeFormats = () => document.querySelectorAll('.tile .fmts').forEach((x) => { x.parentElement!.classList.remove('menuopen'); x.remove(); });
function toggleFormats(tile: HTMLElement, id: string, k: string) {
  const open = tile.querySelector('.fmts');
  closeFormats();
  if (open) return;
  const box = document.createElement('div');
  box.className = 'fmts glass';
  box.innerHTML = exportsFor(id).map((f) => `<button data-f="${f.id}">${f.name}<span>${esc(fmtNote(f.id))}</span></button>`).join('');
  box.onclick = (e) => { const b = (e.target as HTMLElement).closest('button'); if (!b) return; e.stopPropagation(); box.remove(); tile.classList.remove('menuopen'); doExport(b.dataset.f!, id, k); };
  tile.appendChild(box); tile.classList.add('menuopen');
  setTimeout(() => document.addEventListener('click', function off() { box.remove(); tile.classList.remove('menuopen'); document.removeEventListener('click', off); }), 0);
}
/** 嵌入组件还没跟上原版，先不给；GIF 只给会动的作品 */
const EXPORTS = FORMATS.filter((f) => f.id !== 'embed');
const exportsFor = (id: string) => EXPORTS.filter((f) => f.id !== 'gif' || state.style.animated(id));
const fmtNote = (f: string) => ({
  png: L('社交、聊天', 'social, chat'), svg: L('README、网页', 'README, web'), pdf: L('打印、作品集', 'print, portfolio'), gif: L('会动', 'animated'),
  html: L('可交互、离线', 'interactive, offline'), md: 'CREDITS.md', embed: L('网页组件', 'web component'),
} as Record<string, string>)[f] ?? '';

async function doExport(fmtId: string, id: string, k: string) {
  const a = state.arts.get(k)!;
  const m = state.model!;
  const i = Number(k.split('#')[1]);
  const variant = curVariant(id, k, tiles()[i]);
  const base = `${m.project || 'credits'}-${id}${variant ? '-' + variant : ''}-${state.style.id}`;
  try {
    if (fmtId === 'md') { await navigator.clipboard.writeText(mergeCreditsMd(null, creditsBlock(m))).catch(() => null); return toast(L('CREDITS.md 区块已复制', 'CREDITS.md block copied')); }
    toast(fmtId === 'gif' ? L('正在逐帧生成 GIF…', 'Rendering GIF frames…') : L('正在导出…', 'Exporting…'), 60000);
    await new Promise((r) => setTimeout(r, 30));
    const kit = state.style;
    const out = await exportArt(fmtId as Format, kit.style, a, state.size, kit.frames ? () => kit.frames!(state.raw, state.lang, id, variant) : undefined);
    const sz = state.size !== 'native' && fmtId !== 'html' ? `-${state.size}` : '';
    const path = await api('save_file', { name: `${base}${sz}.${out.ext}`, bytes: b64(out.bytes), mime: out.mime });
    toast(L(`已保存 ${path}`, `Saved ${path}`));
  } catch (e: any) { console.error(e); toast(L(`导出失败：${e?.message ?? e}`, `Export failed: ${e?.message ?? e}`), 5000); }
}

function renderDock() {
  const dock = $('#targets');
  dock.innerHTML = EXPORTS.filter((f) => f.id !== 'gif').map((f) => `<button data-f="${f.id}">${f.name}<span>${esc(fmtNote(f.id))}</span></button>`).join('');
  dock.onclick = () => toast(L('把作品拖到这里，或悬停作品点「导出」', 'Drag a piece here, or hover it and click Export'));
  $('#hint').textContent = L('拖动作品到这里导出', 'Drag a piece here to export');
  const sizes = $('#sizes');
  sizes.innerHTML = SIZES.filter((s) => s.id !== 'a4').map((s) => `<button data-s="${s.id}" class="${s.id === state.size ? 'on' : ''}">${s.w ? `${s.w}×${s.h}` : L('原尺寸', 'Native')}</button>`).join('');
}

// ───────────────────────── 拖动 ─────────────────────────
/** 指针在这一排按钮的范围里，就取横向最近的那个：按钮之间的缝、按钮浮起来，都不会让结果来回跳 */
function nearest(row: HTMLElement, x: number, y: number): HTMLElement | null {
  const r = row.getBoundingClientRect();
  if (!r.width || x < r.left - 14 || x > r.right + 14 || y < r.top - 5 || y > r.bottom + 5) return null;
  let best: HTMLElement | null = null, bd = Infinity;
  for (const b of row.querySelectorAll('button')) {
    const q = b.getBoundingClientRect(), d = Math.abs(x - (q.left + q.width / 2));
    if (d < bd) { bd = d; best = b as HTMLElement; }
  }
  return best;
}
const inside = (el: HTMLElement, x: number, y: number, pad = 0) => { const r = el.getBoundingClientRect(); return x > r.left - pad && x < r.right + pad && y > r.top - pad && y < r.bottom + pad; };

function startDrag(e: PointerEvent, tile: HTMLElement, id: string, k: string) {
  if (e.button !== 0) return;
  const x0 = e.clientX, y0 = e.clientY;
  let dragging = false;
  const ghost = $('#ghost');
  let over: HTMLElement | null = null;
  // 作品抓在按下的那一点上跟手走；移到导出按钮上就缩小，让出按钮
  const r0 = (e.currentTarget as HTMLElement).getBoundingClientRect();
  const fx = (x0 - r0.left) / r0.width, fy = (y0 - r0.top) / r0.height;
  const move = (ev: PointerEvent) => {
    if (!dragging && Math.hypot(ev.clientX - x0, ev.clientY - y0) > 6) {
      dragging = true;
      document.body.classList.add('dragging');
      const a = state.arts.get(k)!;
      const w = Math.min(240, a.w);
      ghost.style.width = `${w}px`;
      ghost.style.transformOrigin = `${fx * 100}% ${fy * 100}%`;
      ghost.innerHTML = a.svg;
      ghost.hidden = false;
      ghost.animate([{ scale: String(r0.width / w) }, { scale: '1' }], { duration: 220, easing: 'cubic-bezier(.2,.8,.2,1)' });
      $('#hint').textContent = L('松手导出 · 选好尺寸', 'Drop to export · pick a size');
      document.querySelector('.ct-tip')?.classList.remove('on');
    }
    if (!dragging) return;
    ghost.style.translate = `${ev.clientX - ghost.offsetWidth * fx}px ${ev.clientY - ghost.offsetHeight * fy}px`; // 用单独的 translate，缩放才不会连位置一起缩
    const tb = nearest($('#targets'), ev.clientX, ev.clientY);
    const sz = tb ? null : nearest($('#sizes'), ev.clientX, ev.clientY);
    if (sz && sz.dataset.s !== state.size) { state.size = sz.dataset.s!; store.set('size', state.size); document.querySelectorAll('#sizes button').forEach((b) => b.classList.toggle('on', b === sz)); }
    if (tb !== over) { over?.classList.remove('over'); over = tb; over?.classList.add('over'); }
    // 进了导出坞就缩小让出按钮；整块坞都算，划过按钮之间的缝不会忽大忽小
    ghost.classList.toggle('small', inside($('#dock'), ev.clientX, ev.clientY, 16));
  };
  const up = (ev: PointerEvent) => {
    removeEventListener('pointermove', move); removeEventListener('pointerup', up); removeEventListener('pointercancel', up);
    if (!dragging) return ev.type === 'pointerup' ? openFocus(id, k) : undefined;
    if (ev.type === 'pointercancel') { over?.classList.remove('over'); over = null; }
    document.body.classList.remove('dragging');
    ghost.hidden = true; ghost.innerHTML = ''; ghost.classList.remove('small');
    $('#hint').textContent = L('拖动作品到这里导出', 'Drag a piece here to export');
    if (over) { const f = over.dataset.f!; over.classList.remove('over'); doExport(f, id, k); }
  };
  addEventListener('pointermove', move); addEventListener('pointerup', up); addEventListener('pointercancel', up);
}

// ───────────────────────── 聚焦：大图 + 全部版式 ─────────────────────────
/** 关掉最上面那一层：Esc 只是个快捷键，每一层都有看得见的关闭按钮，点空白处也能关 */
let closeFocus: (() => void) | null = null;
function closeTop() {
  if (!$('#sheet').hidden) return closeSheet();
  if (closeFocus) return closeFocus();
  if (!$('#menu').hidden) return closeMenu();
  closeFormats();
}
const closeBtn = () => `<button class="closer glass" data-act="close" aria-label="${L('关闭', 'Close')}"><svg width="12" height="12" viewBox="0 0 12 12"><path d="M2.5 2.5l7 7M9.5 2.5l-7 7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>${L('关闭', 'Close')}<kbd>Esc</kbd></button>`;

function openFocus(id: string, k: string) {
  const el = $('#focus');
  const vs = state.style.variants(id);
  const i = Number(k.split('#')[1]);
  const before = curVariant(id, k, tiles()[i]);
  const draw = async () => {
    const variant = curVariant(id, k, tiles()[i]);
    const a = await drawArt(id, variant);
    if (!a) return;
    state.arts.set(k, a);
    el.innerHTML = `${closeBtn()}
      <div class="big"><div class="art"></div></div>
      <div class="strip glass">
        <b style="padding:0 8px">${esc(state.style.pieceName(id, state.lang))}</b><span style="color:var(--soft)">${esc(state.style.pieceQ(id, state.lang))}</span>
        ${vs.length > 1 ? `<span class="sep"></span><span class="lab">${L('版式', 'Layout')}</span>${vs.map((v) => `<button data-v="${v}" class="${v === variant ? 'on' : ''}">${esc(state.style.variantName(v, state.lang))}</button>`).join('')}` : ''}
        <span class="sep"></span><span class="lab">${L('导出', 'Export')}</span>${exportsFor(id).map((f) => `<button data-f="${f.id}">${f.name}</button>`).join('')}
      </div>`;
    const artEl = el.querySelector('.art') as HTMLElement;
    artEl.innerHTML = a.svg;
    (el.querySelector('.big') as HTMLElement).style.width = `${Math.max(320, Math.min(a.w, 1100, ((innerHeight - 240) * a.w) / a.h))}px`;
    mountHits(artEl, a.hits, a.w, a.h);
  };
  draw();
  el.hidden = false;
  el.onclick = (e) => {
    const b = (e.target as HTMLElement).closest('button') as HTMLElement | null;
    if (!b) { if (e.target === el) close(); return; }
    if (b.dataset.act === 'close') return close();
    if (b.dataset.v) { state.variants[k] = b.dataset.v; draw(); return; }
    if (b.dataset.f) doExport(b.dataset.f, id, k);
  };
  // 只有换过版式才把墙上那一张换掉，别的作品不动
  const close = () => {
    el.hidden = true; el.innerHTML = ''; closeFocus = null;
    document.querySelector('.ct-tip')?.classList.remove('on');
    if (curVariant(id, k, tiles()[i]) !== before) { const t = document.querySelector(`.tile[data-k="${k}"]`) as HTMLElement | null; if (t) repaintTile(t, id, k); }
  };
  closeFocus = close;
}

// ───────────────────────── 风格圆点 ─────────────────────────
function renderDots() {
  const nav = $('#dots');
  // 原版主题和新风格之间留一道细线
  nav.innerHTML = KITS.map((t, i) => `${i === CLASSIC_COUNT ? '<i class="divider"></i>' : ''}<button data-s="${t.id}" class="${t.id === state.style.id ? 'on' : ''}" aria-label="${esc(t.name(state.lang))}"><svg viewBox="0 0 24 24">${t.dot}</svg></button>`).join('');
  const tipEl = $('#tipname');
  nav.onmouseover = (e) => {
    const b = (e.target as HTMLElement).closest('button') as HTMLElement | null; if (!b) return;
    const t = kitById(b.dataset.s!)!;
    tipEl.innerHTML = `<b>${esc(t.name(state.lang))}</b><span>${esc(t.note(state.lang))}</span>`;
    tipEl.hidden = false;
    const r = b.getBoundingClientRect();
    tipEl.style.left = `${Math.max(8, r.left - 10)}px`; tipEl.style.top = `${r.top - tipEl.offsetHeight - 12}px`;
  };
  nav.onmouseleave = () => (tipEl.hidden = true);
  nav.onclick = async (e) => {
    const b = (e.target as HTMLElement).closest('button') as HTMLElement | null; if (!b) return;
    const t = kitById(b.dataset.s!) as Kit;
    if (t.id === state.style.id) return;
    state.style = t; store.set('theme', t.id);
    nav.querySelectorAll('button').forEach((x) => x.classList.toggle('on', x === b));
    state.variants = {};
    await paint();
  };
}

// ───────────────────────── 顶栏：项目、语言、设置 ─────────────────────────
function wireTop() {
  const pill = $('#project');
  pill.onclick = () => toggleMenu();
  pill.onkeydown = (e) => { if (e.key === 'Enter' || e.key === ' ') toggleMenu(); };
  const lang = $('#lang'), gear = $('#gear');
  const labels = () => {
    lang.textContent = state.lang === 'zh' ? '中' : 'EN';
    lang.title = L('界面语言：中文（点一下换成 English）', 'Language: English (click for 中文)');
    gear.title = L('设置', 'Settings'); gear.setAttribute('aria-label', gear.title);
  };
  labels();
  addEventListener('keydown', (e) => { if (e.key === 'Escape') closeTop(); });
  lang.onclick = async () => {
    state.lang = state.lang === 'zh' ? 'en' : 'zh'; store.set('lang', state.lang);
    labels();
    document.documentElement.lang = state.lang;
    renderDock(); renderDots();
    if (state.raw) { rebuildModel(); await paint(); }
  };
  gear.onclick = () => settings();
}
function closeMenu() { $('#menu').hidden = true; $('#project').setAttribute('aria-expanded', 'false'); }
function toggleMenu() {
  const menu = $('#menu'), pill = $('#project');
  if (!menu.hidden) return closeMenu();
  pill.setAttribute('aria-expanded', 'true');
  menu.hidden = false;
  menu.innerHTML = `<input id="q" placeholder="${L('搜索项目', 'Search projects')}" autocomplete="off"><div class="list" id="plist"></div>
    <div class="foot"><button id="add">＋ ${L('添加项目', 'Add project')}</button><button id="re">↻ ${L('重新分析', 'Re-analyze')}</button></div>`;
  const draw = () => {
    const q = ($('#q') as HTMLInputElement).value.trim().toLowerCase();
    $('#plist').innerHTML = state.projects.filter((p) => !q || `${p.name} ${p.path}`.toLowerCase().includes(q)).map((p) => `
      <div class="item ${p.id === state.current?.id ? 'on' : ''}" data-id="${esc(p.id)}">
        <b>${esc(p.name)}</b><span class="r">${p.lastAnalyzed ? new Date(p.lastAnalyzed * 1000).toLocaleDateString(state.lang === 'zh' ? 'zh-CN' : 'en-US') : L('未分析', 'not analyzed')}</span>
        <span>${esc(p.path)}</span><span class="r">${p.evidence ? `${fmt(p.evidence)} ${L('条证据', 'evidence')}` : ''}</span>
        ${p.paths?.length > 1 ? `<span class="paths">${L('还在', 'also at')} ${esc(p.paths.slice(1, 3).join('、'))}${p.paths.length > 3 ? ' …' : ''}</span>` : ''}
      </div>`).join('') || `<div class="item"><span>${L('没有匹配的项目', 'No match')}</span></div>`;
  };
  draw();
  ($('#q') as HTMLInputElement).oninput = draw;
  ($('#q') as HTMLInputElement).focus();
  $('#plist').onclick = (e) => {
    const it = (e.target as HTMLElement).closest('.item') as HTMLElement | null; if (!it?.dataset.id) return;
    menu.hidden = true; pill.setAttribute('aria-expanded', 'false');
    const p = state.projects.find((x) => x.id === it.dataset.id);
    if (p && p !== state.current) openProject(p);
  };
  $('#add').onclick = async () => {
    try { const r = await api('add_project'); if (r) { state.projects = await api('projects'); const p = state.projects.find((x) => x.id === r.id); if (p) openProject(p); menu.hidden = true; } }
    catch (e: any) { toast(String(e?.message ?? e)); }
  };
  $('#re').onclick = async () => {
    if (!state.current) return;
    toast(L('正在分析…', 'Analyzing…'), 120000);
    try { await api('analyze', { projectId: state.current.id }); state.projects = await api('projects'); await openProject(state.projects.find((x) => x.id === state.current.id) ?? state.current); toast(L('已更新', 'Updated')); }
    catch (e: any) { toast(String(e?.message ?? e), 5000); }
  };
  setTimeout(() => document.addEventListener('click', function off(e) { if (!menu.contains(e.target as Node) && !pill.contains(e.target as Node)) { menu.hidden = true; pill.setAttribute('aria-expanded', 'false'); document.removeEventListener('click', off); } }), 0);
}

function closeSheet() { const el = $('#sheet'); el.hidden = true; el.innerHTML = ''; }
async function settings() {
  const el = $('#sheet');
  el.hidden = false;
  el.innerHTML = `<div class="panel glass">
    <div class="head"><h2>${L('设置', 'Settings')}</h2>${closeBtn()}</div>
    <h3>${L('数据源', 'Data sources')}</h3><div id="doc"><p>${L('检查中…', 'Checking…')}</p></div>
    <h3>${L('关于', 'About')}</h3>
    <p>${L('分析在本机完成：读各工具的日志和 git 历史，只保存行的哈希和时间、模型这类信息，不保存代码和 prompt，也不调用任何模型。', 'Analysis runs on this machine: it reads tool logs and git history, keeps only line hashes and metadata such as time and model, never code or prompts, and calls no models.')}</p>
  </div>`;
  el.onclick = (e) => { if (e.target === el || (e.target as HTMLElement).closest('[data-act="close"]')) closeSheet(); };
  const doc = await api('doctor').catch(() => []);
  $('#doc').innerHTML = doc.length ? doc.map((d: any) => `<div class="row"><i class="dot ${d.ok ? 'ok' : ''}"></i><span class="k">${esc(d.name)}</span><span class="v">${esc(d.detail)}</span></div>`).join('') : `<p>${L('没有可用的数据源', 'No data sources found')}</p>`;
}

boot();
