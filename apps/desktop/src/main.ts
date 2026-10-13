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
  $('#board').innerHTML = '';
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
  board.innerHTML = '';
  state.arts.clear();
  // 一张一张画，画好一张放一张；切了项目或主题就停
  for (const [i, b] of tiles().entries()) {
    const k = keyOf(b, i);
    const variant = curVariant(b.id, k, b);
    let a: Art | null = null;
    try { a = await drawArt(b.id, variant); } catch (e) { console.error(b.id, e); }
    if (token !== state.token) return;
    if (!a) continue;
    state.arts.set(k, a);
    const tile = document.createElement('div');
    tile.className = 'tile';
    tile.dataset.k = k;
    tile.dataset.span = String(b.span ?? (a.w / a.h > 2.2 ? 2 : 1));
    const vs = st.variants(b.id);
    tile.innerHTML = tileHtml(b.id, vs, variant);
    const artEl = tile.querySelector('.art') as HTMLElement;
    artEl.innerHTML = a.svg;
    mountHits(artEl, a.hits, a.w, a.h);
    wireTile(tile, b.id, k, vs, variant ?? '');
    board.appendChild(tile);
    layout();
  }
}
function tileHtml(id: string, vs: string[], cur?: string) {
  return `<div class="name">${esc(state.style.pieceName(id, state.lang))}</div>
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
    const a = state.arts.get(t.dataset.k!)!;
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
  fresh.className = 'tile'; fresh.dataset.k = k; fresh.dataset.span = String(b.span ?? (a.w / a.h > 2.2 ? 2 : 1)); fresh.style.animation = 'none';
  fresh.innerHTML = tileHtml(id, vs, variant);
  const artEl = fresh.querySelector('.art') as HTMLElement;
  artEl.innerHTML = a.svg;
  mountHits(artEl, a.hits, a.w, a.h);
  wireTile(fresh, id, k, vs, variant ?? '');
  tile.replaceWith(fresh);
  layout();
}

// ───────────────────────── 导出 ─────────────────────────
function toggleFormats(tile: HTMLElement, id: string, k: string) {
  const open = tile.querySelector('.fmts');
  document.querySelectorAll('.tile .fmts').forEach((x) => { x.parentElement!.classList.remove('menuopen'); x.remove(); });
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
function startDrag(e: PointerEvent, tile: HTMLElement, id: string, k: string) {
  if (e.button !== 0) return;
  const x0 = e.clientX, y0 = e.clientY;
  let dragging = false;
  const ghost = $('#ghost');
  let over: HTMLElement | null = null;
  const move = (ev: PointerEvent) => {
    if (!dragging && Math.hypot(ev.clientX - x0, ev.clientY - y0) > 6) {
      dragging = true;
      document.body.classList.add('dragging');
      const a = state.arts.get(k)!;
      const w = Math.min(280, a.w);
      ghost.style.width = `${w}px`;
      ghost.innerHTML = a.svg;
      ghost.hidden = false;
      $('#hint').textContent = L('松手导出 · 选好尺寸', 'Drop to export · pick a size');
      document.querySelector('.ct-tip')?.classList.remove('on');
    }
    if (!dragging) return;
    ghost.style.transform = `translate(${ev.clientX - ghost.offsetWidth - 18}px, ${ev.clientY - ghost.offsetHeight * 0.6}px) rotate(-2deg)`;
    const t = (document.elementFromPoint(ev.clientX, ev.clientY) as HTMLElement | null)?.closest('#targets button, #sizes button') as HTMLElement | null;
    if (t?.dataset.s) { state.size = t.dataset.s; store.set('size', state.size); document.querySelectorAll('#sizes button').forEach((b) => b.classList.toggle('on', b === t)); }
    const tb = t?.dataset.f ? t : null;
    if (tb !== over) { over?.classList.remove('over'); over = tb; over?.classList.add('over'); }
  };
  const up = () => {
    removeEventListener('pointermove', move); removeEventListener('pointerup', up);
    if (!dragging) return openFocus(id, k);
    document.body.classList.remove('dragging');
    ghost.hidden = true; ghost.innerHTML = '';
    $('#hint').textContent = L('拖动作品到这里导出', 'Drag a piece here to export');
    if (over) { const f = over.dataset.f!; over.classList.remove('over'); doExport(f, id, k); }
  };
  addEventListener('pointermove', move); addEventListener('pointerup', up);
}

// ───────────────────────── 聚焦：大图 + 全部版式 ─────────────────────────
function openFocus(id: string, k: string) {
  const el = $('#focus');
  const vs = state.style.variants(id);
  const i = Number(k.split('#')[1]);
  const draw = async () => {
    const variant = curVariant(id, k, tiles()[i]);
    const a = await drawArt(id, variant);
    if (!a) return;
    state.arts.set(k, a);
    el.innerHTML = `<button class="round glass close" data-act="close">✕</button>
      <div class="big"><div class="art"></div></div>
      <div class="strip glass">
        <b style="padding:0 8px">${esc(state.style.pieceName(id, state.lang))}</b><span style="color:var(--soft)">${esc(state.style.pieceQ(id, state.lang))}</span>
        ${vs.length > 1 ? `<span class="sep"></span>${vs.map((v) => `<button data-v="${v}" class="${v === variant ? 'on' : ''}">${esc(state.style.variantName(v, state.lang))}</button>`).join('')}` : ''}
        <span class="sep"></span>${exportsFor(id).map((f) => `<button data-f="${f.id}">${f.name}</button>`).join('')}
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
  const close = () => { el.hidden = true; el.innerHTML = ''; document.removeEventListener('keydown', key); paint(); };
  const key = (e: KeyboardEvent) => { if (e.key === 'Escape') close(); };
  document.addEventListener('keydown', key);
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
  const lang = $('#lang');
  lang.textContent = state.lang === 'zh' ? '中' : 'EN';
  lang.onclick = async () => {
    state.lang = state.lang === 'zh' ? 'en' : 'zh'; store.set('lang', state.lang);
    lang.textContent = state.lang === 'zh' ? '中' : 'EN';
    document.documentElement.lang = state.lang;
    renderDock(); renderDots();
    if (state.raw) { rebuildModel(); await paint(); }
  };
  $('#gear').onclick = () => settings();
}
function toggleMenu() {
  const menu = $('#menu'), pill = $('#project');
  if (!menu.hidden) { menu.hidden = true; pill.setAttribute('aria-expanded', 'false'); return; }
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

async function settings() {
  const el = $('#sheet');
  el.hidden = false;
  el.innerHTML = `<div class="panel glass">
    <h2>${L('设置', 'Settings')}</h2>
    <h3>${L('数据源', 'Data sources')}</h3><div id="doc"><p>${L('检查中…', 'Checking…')}</p></div>
    <h3>${L('关于', 'About')}</h3>
    <p>${L('分析在本机完成：读各工具的日志和 git 历史，只保存行的哈希和时间、模型这类信息，不保存代码和 prompt，也不调用任何模型。', 'Analysis runs on this machine: it reads tool logs and git history, keeps only line hashes and metadata such as time and model, never code or prompts, and calls no models.')}</p>
  </div>`;
  el.onclick = (e) => { if (e.target === el) { el.hidden = true; el.innerHTML = ''; } };
  const doc = await api('doctor').catch(() => []);
  $('#doc').innerHTML = doc.length ? doc.map((d: any) => `<div class="row"><i class="dot ${d.ok ? 'ok' : ''}"></i><span class="k">${esc(d.name)}</span><span class="v">${esc(d.detail)}</span></div>`).join('') : `<p>${L('没有可用的数据源', 'No data sources found')}</p>`;
}

boot();
