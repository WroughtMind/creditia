// creditia App：一面展墙。顶部切换项目，左下角圆点切换风格，所有作品直接摆出来；
// 悬停看细节、换版式、选格式导出；拖到右下角的导出坞也能导出。
import { buildModel, type Model } from '../../../packages/render/src/classic/data.ts';
import { nameOf } from '../../../packages/render/src/data.ts';
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
type Act = { label: string; run: () => void };
let toastTimer = 0, toastMs = 0;
/** 提示条：可以带按钮（在访达中显示、取消……）和转圈；ms 为 0 就一直留着。鼠标停在上面时不消失 */
function toast(msg: string, ms = 2600, o: { acts?: Act[]; spin?: boolean } = {}) {
  const el = $('#toast');
  el.innerHTML = `${o.spin ? '<i class="spin"></i>' : ''}<span class="msg"></span>${(o.acts ?? []).map((a, i) => `<button data-i="${i}">${esc(a.label)}</button>`).join('')}`;
  el.querySelector('.msg')!.textContent = msg;
  el.onclick = (e) => { const b = (e.target as HTMLElement).closest('button'); if (!b) return; el.hidden = true; o.acts![Number(b.dataset.i)].run(); };
  el.hidden = false;
  toastMs = ms; clearTimeout(toastTimer);
  if (ms > 0) toastTimer = setTimeout(() => (el.hidden = true), ms);
}
/** 原生端的错误只带代码和细节，文案在这里按语言写 */
const ERRORS: Record<string, [string, string]> = {
  not_in_library: ['这个项目不在作品库里', 'This project is not in the library'],
  no_credits: ['这个项目还没有档案', 'No credits for this project yet'],
  dir_missing: ['项目目录已经不在了', 'The project folder is gone'],
  core_missing: ['找不到分析核心 creditia-core', 'Cannot find the analysis core (creditia-core)'],
  core_failed: ['分析失败', 'Analysis failed'],
  busy: ['已经有一个分析在跑', 'An analysis is already running'],
  cancelled: ['分析已取消', 'Analysis cancelled'],
  no_home: ['找不到用户目录', 'Cannot find your home folder'],
  io: ['读写文件失败', 'Could not read or write the file'],
  store: ['读不到作品库', 'Cannot read the library'],
  not_allowed: ['只能打开这次导出的文件', 'Only files exported in this session can be shown'],
};
function errText(e: any): string {
  const t = e?.code && ERRORS[e.code], d = e?.detail ? String(e.detail) : '';
  if (t) return L(t[0], t[1]) + (d ? L(`：${d}`, `: ${d}`) : '');
  return String(e?.message ?? e);
}
/** 家目录缩成 ~ */
const short = (path: string) => path.replace(/^(\/Users|\/home)\/[^/]+/, '~').replace(/^[A-Za-z]:\\Users\\[^\\]+/, '~');
const OS = /Mac/.test(navigator.userAgent) ? 'mac' : /Windows/.test(navigator.userAgent) ? 'win' : 'linux';
const revealLabel = () => (OS === 'mac' ? L('在访达中显示', 'Show in Finder') : OS === 'win' ? L('在资源管理器中显示', 'Show in Explorer') : L('打开所在文件夹', 'Open folder'));

const tiles = () => state.style.board;
const keyOf = (b: { id: string; variant?: string }, i: number) => `${b.id}#${i}`;

// ───────────────────────── 启动 ─────────────────────────
async function boot() {
  bootKits();
  const st = document.createElement('style'); st.textContent = INTERACT_CSS; document.head.appendChild(st);
  $('#logo').innerHTML = LOGO_SVG;
  applyLang(); renderDots(); renderDock(); wireTop();
  try { state.projects = await api('projects'); } catch (e: any) { return empty(() => [L('读不到作品库', 'Cannot read the library'), errText(e)]); }
  if (!state.projects.length) return empty(() => [L('还没有分析过的项目', 'No analyzed projects yet'), '', 'creditia analyze', native ? { label: L('添加项目…', 'Add a project…'), run: addProject } : undefined]);
  const last = state.projects.find((p) => p.id === store.get('project')) ?? state.projects[0];
  await openProject(last);
  addEventListener('resize', () => layout());
}
/** 空状态。传进来的是函数：换语言时按新语言重写一遍 */
type EmptyView = () => [title: string, detail?: string, cmd?: string, act?: Act];
let emptyView: EmptyView | null = null;
function empty(view: EmptyView, keepName = false) {
  emptyView = view;
  const [title, detail = '', cmd = '', act] = view();
  $('#board').innerHTML = ''; document.body.classList.remove('busy');
  const el = $('#empty'); el.hidden = false;
  el.innerHTML = `<div><b>${esc(title)}</b>${esc(detail)}${act ? `<br><button class="go glass">${esc(act.label)}</button>` : ''}${cmd ? `<br><span>${act ? L('或者在项目目录里运行', 'or run in your project') : L('在项目目录里运行', 'Run in your project')}</span><br><code>${esc(cmd)}</code>` : ''}</div>`;
  if (act) (el.querySelector('.go') as HTMLElement).onclick = act.run;
  if (!keepName) { $('#pname').textContent = 'creditia'; $('#pmeta').textContent = ''; }
}

async function openProject(p: any) {
  const seq = ++state.openSeq;
  state.current = p; store.set('project', p.id);
  state.raw = null; state.model = null; state.arts.clear(); state.variants = {};
  ++state.token; // 让正在画的旧项目停下
  $('#board').innerHTML = '';
  $('#pname').textContent = p.name; $('#pmeta').textContent = L('读取中…', 'Loading…');
  let raw: any;
  const st = readStamp(p); // 先记下版本再读档案：读的过程中又变了，下次比对还能发现
  try { raw = await api('credits', { projectId: p.id }); }
  catch (e: any) {
    if (seq !== state.openSeq) return;
    $('#pmeta').textContent = '';
    if (e?.code && e.code !== 'no_credits') return empty(() => [L('读不到这个项目的档案', 'Cannot read this project'), errText(e)], true);
    const can = native && p.exists !== false;
    return empty(() => [L('这个项目还没有档案', 'No credits for this project yet'), '', `cd ${p.path} && creditia analyze`, can ? { label: L('现在分析', 'Analyze now'), run: () => reanalyze(p) } : undefined], true);
  }
  if (seq !== state.openSeq) return; // 已经切到别的项目
  stampOf = await st;
  state.raw = raw;
  $('#empty').hidden = true; emptyView = null;
  rebuildModel();
  await paint();
}

// ───────────────────────── 档案变了就原地刷新 ─────────────────────────
// 在终端里 creditia analyze、提交时的 hook 都会改档案。App 回到前台、或者每 30 秒，比一下版本；
// 变了就读新档案，墙上的作品原地换掉（不清空、不跳回顶部），换过的版式也留着。
let stampOf = '', refreshing = false;
const readStamp = (p: any): Promise<string> => (native ? api('stamp', { projectId: p.id }).then(String, () => '') : Promise.resolve(''));
async function refreshIfChanged() {
  const p = state.current;
  if (!native || !p || !state.raw || refreshing || analyzing || document.hidden) return;
  refreshing = true;
  try {
    const st = await readStamp(p);
    if (!st || st === stampOf || p !== state.current) return;
    const raw = await api('credits', { projectId: p.id });
    if (p !== state.current) return;
    stampOf = st;
    await swapRaw(raw);
    state.projects = await api('projects').catch(() => state.projects);
    toast(L('档案有更新，已换成最新的', 'Credits changed — showing the latest'));
  } catch { /* 读不到就等下一次 */ } finally { refreshing = false; }
}
/** 换一份档案，墙上原地换 */
async function swapRaw(raw: any) {
  state.raw = raw;
  rebuildModel();
  await paint();
}
addEventListener('focus', refreshIfChanged);
document.addEventListener('visibilitychange', refreshIfChanged);
setInterval(refreshIfChanged, 30000);
function rebuildModel() {
  state.model = buildModel(state.raw, { director: false, lang: state.lang });
  const m = state.model;
  $('#pname').textContent = m.project || state.current?.name;
  $('#pmeta').textContent = `${fmt(m.commits)} ${L('次提交', 'commits')} · ${fmt(m.linesAdded)} ${L('行', 'lines')}`;
}

// ───────────────────────── 展墙 ─────────────────────────
const curVariant = (id: string, k: string, b: { variant?: string }) => state.style.current(id, state.variants[k] ?? b.variant);

/** 画过的作品按「档案 + 风格 + 语言 + 作品 + 版式」记住：切回看过的风格、来回换版式都不用再画。
 *  存的是 Promise，同一张正在画的时候墙和放大视图共用一次；按 SVG 总长度限量，最久没用的先丢 */
const cache = new Map<string, Promise<Art | null>>();
const cacheLen = new Map<string, number>();
let cacheRaw: any = null, cacheTotal = 0;
const CACHE_MAX = 32e6; // SVG 字符数
function drawArt(id: string, variant?: string): Promise<Art | null> {
  if (cacheRaw !== state.raw) { cache.clear(); cacheLen.clear(); cacheTotal = 0; cacheRaw = state.raw; }
  const key = `${state.style.id}|${state.lang}|${id}|${variant ?? ''}`;
  let hit = cache.get(key);
  if (hit) { cache.delete(key); cache.set(key, hit); return hit; } // 挪到最近用过
  hit = state.style.render(state.raw, state.lang, id, variant);
  cache.set(key, hit);
  hit.then((a) => {
    if (cache.get(key) !== hit) return;
    const n = a?.svg.length ?? 0;
    cacheLen.set(key, n); cacheTotal += n;
    for (const old of cache.keys()) {
      if (cacheTotal <= CACHE_MAX) break;
      if (old === key) continue;
      cacheTotal -= cacheLen.get(old) ?? 0; cacheLen.delete(old); cache.delete(old);
    }
  }, () => { if (cache.get(key) === hit) cache.delete(key); });
  return hit;
}

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
    tileA11y(tile, b.id);
    tile.dataset.k = k;
    tile.dataset.w = String(a.w); tile.dataset.h = String(a.h);
    tile.dataset.span = String(b.span ?? (a.w / a.h > 2.2 ? 2 : 1));
    const vs = st.variants(b.id);
    tile.innerHTML = tileHtml(b.id, vs, variant);
    const artEl = tile.querySelector('.art') as HTMLElement;
    artEl.innerHTML = a.svg;
    mountHits(artEl, a.hits, a.w, a.h);
    wireTile(tile, b.id, k, vs);
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

function wireTile(tile: HTMLElement, id: string, k: string, vs: string[]) {
  tile.querySelector('.bar')!.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest('button'); if (!b) return;
    e.stopPropagation();
    const act = b.dataset.act;
    if (act === 'prev' || act === 'next') {
      // 从现在选中的算起（不是这张画出来时的），连点两下就走两步；名字先换，图画好再换
      const i = vs.indexOf(curVariant(id, k, tiles()[Number(k.split('#')[1])]) ?? '');
      const v = vs[(i + (act === 'next' ? 1 : vs.length - 1)) % vs.length];
      state.variants[k] = v;
      const name = tile.querySelector('.vname'); if (name) name.textContent = state.style.variantName(v, state.lang);
      return repaintTile(id, k);
    }
    if (act === 'export') toggleFormats(tile, id, k);
  });
  const artEl = tile.querySelector('.art') as HTMLElement;
  artEl.addEventListener('pointerdown', (e) => startDrag(e, tile, id, k));
  tile.addEventListener('keydown', (e) => { if (e.target === tile && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); openFocus(id, k); } });
}
/** 键盘也能用：Tab 走到每一张，回车放大 */
function tileA11y(tile: HTMLElement, id: string) {
  tile.tabIndex = 0;
  tile.setAttribute('role', 'button');
  tile.setAttribute('aria-label', `${state.style.pieceName(id, state.lang)} · ${L('回车放大', 'press Enter to enlarge')}`);
}
/** 只重画一张（换版式）。连着换的时候只认最后一次，换下来的是墙上现在那一张 */
const tileSeq = new Map<string, number>();
async function repaintTile(id: string, k: string) {
  const i = Number(k.split('#')[1]);
  const b = tiles()[i];
  const variant = curVariant(id, k, b);
  const token = state.token, seq = (tileSeq.get(k) ?? 0) + 1;
  tileSeq.set(k, seq);
  const a = await drawArt(id, variant);
  if (!a || token !== state.token || tileSeq.get(k) !== seq) return;
  const tile = document.querySelector(`.tile[data-k="${k}"]`) as HTMLElement | null;
  if (!tile) return;
  state.arts.set(k, a);
  const vs = state.style.variants(id);
  const fresh = document.createElement('div');
  fresh.className = 'tile placed'; fresh.dataset.k = k; fresh.dataset.w = String(a.w); fresh.dataset.h = String(a.h); fresh.style.left = tile.style.left; fresh.style.top = tile.style.top; fresh.style.width = tile.style.width; fresh.dataset.span = String(b.span ?? (a.w / a.h > 2.2 ? 2 : 1)); fresh.style.animation = 'none';
  tileA11y(fresh, id);
  fresh.innerHTML = tileHtml(id, vs, variant);
  const artEl = fresh.querySelector('.art') as HTMLElement;
  artEl.innerHTML = a.svg;
  mountHits(artEl, a.hits, a.w, a.h);
  wireTile(fresh, id, k, vs);
  if (tile.classList.contains('hover')) fresh.classList.add('hover');
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
    if (fmtId === 'md') {
      const text = mergeCreditsMd(null, creditsBlock(m));
      try { await navigator.clipboard.writeText(text); return toast(L('CREDITS.md 区块已复制', 'CREDITS.md block copied')); }
      catch { return toast(L('没能写进剪贴板', 'Could not copy to the clipboard'), 8000, { acts: [{ label: L('存成文件', 'Save as a file'), run: () => saveBytes('CREDITS.md', new TextEncoder().encode(text), 'text/markdown').catch((e) => toast(errText(e), 5000)) }] }); }
    }
    toast(fmtId === 'gif' ? L('正在逐帧生成 GIF…', 'Rendering GIF frames…') : L('正在导出…', 'Exporting…'), 60000, { spin: true });
    await new Promise((r) => setTimeout(r, 30));
    const kit = state.style;
    const out = await exportArt(fmtId as Format, kit.style, a, state.size, kit.frames ? () => kit.frames!(state.raw, state.lang, id, variant) : undefined);
    const sz = state.size !== 'native' && fmtId !== 'html' ? `-${state.size}` : '';
    await saveBytes(`${base}${sz}.${out.ext}`, out.bytes, out.mime);
  } catch (e: any) { console.error(e); toast(L(`导出失败：${errText(e)}`, `Export failed: ${errText(e)}`), 5000); }
}
/** 存文件；App 里提示带「在访达中显示」 */
async function saveBytes(name: string, bytes: Uint8Array, mime: string) {
  const path: string = await api('save_file', { name, bytes: b64(bytes), mime });
  const acts = native ? [{ label: revealLabel(), run: () => { api('reveal', { path }).catch((e) => toast(errText(e), 5000)); } }] : [];
  toast(L(`已保存到 ${short(path)}`, `Saved to ${short(path)}`), 6000, { acts });
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
/** 放大视图开着时的方向键：← → 换版式，↑ ↓ 换作品 */
let focusKeys: ((e: KeyboardEvent) => boolean) | null = null;
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
  let seq = 0, open = true;
  // 快速连点几个版式：按钮立刻跟上，图只认最后一次（先点的画得慢，回来也不会盖掉后点的）
  const draw = async () => {
    const my = ++seq;
    const variant = curVariant(id, k, tiles()[i]);
    el.querySelectorAll<HTMLElement>('.strip button[data-v]').forEach((b) => b.classList.toggle('on', b.dataset.v === variant));
    const a = await drawArt(id, variant);
    if (!a || my !== seq || !open) return;
    state.arts.set(k, a);
    el.innerHTML = `${closeBtn()}
      <div class="big"><div class="art"></div></div>
      <div class="strip glass">
        <b style="padding:0 8px">${esc(state.style.pieceName(id, state.lang))}</b><span style="color:var(--soft)">${esc(state.style.pieceQ(id, state.lang))}</span><span class="keys" title="${L('上一件 / 下一件作品', 'Previous / next piece')}"><kbd>↑</kbd><kbd>↓</kbd></span>
        ${vs.length > 1 ? `<span class="sep"></span><span class="lab">${L('版式', 'Layout')}</span><span class="keys" title="${L('上一种 / 下一种版式', 'Previous / next layout')}"><kbd>←</kbd><kbd>→</kbd></span>${vs.map((v) => `<button data-v="${v}" class="${v === variant ? 'on' : ''}">${esc(state.style.variantName(v, state.lang))}</button>`).join('')}` : ''}
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
  // 只有换过版式才把墙上那一张换掉，别的作品不动；键盘焦点回到墙上这一张
  const close = (refocus = true) => {
    open = false; el.hidden = true; el.innerHTML = ''; closeFocus = null; focusKeys = null;
    document.querySelector('.ct-tip')?.classList.remove('on');
    if (curVariant(id, k, tiles()[i]) !== before) repaintTile(id, k);
    if (refocus) (document.querySelector(`.tile[data-k="${k}"]`) as HTMLElement | null)?.focus({ preventScroll: true });
  };
  closeFocus = close;
  focusKeys = (e) => {
    if (e.metaKey || e.ctrlKey || e.altKey) return false;
    if ((e.key === 'ArrowLeft' || e.key === 'ArrowRight') && vs.length > 1) {
      const j = vs.indexOf(curVariant(id, k, tiles()[i]) ?? '');
      state.variants[k] = vs[(j + (e.key === 'ArrowRight' ? 1 : vs.length - 1)) % vs.length];
      draw();
      return true;
    }
    if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      const all = [...document.querySelectorAll<HTMLElement>('#board .tile')];
      const at = all.findIndex((t) => t.dataset.k === k);
      const next = all[at + (e.key === 'ArrowDown' ? 1 : -1)];
      if (next) { close(false); openFocus(next.dataset.k!.split('#')[0], next.dataset.k!); next.scrollIntoView({ block: 'nearest' }); }
      return true;
    }
    return false;
  };
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
/** 页面里写死的字、无障碍标签、<html lang> */
function applyLang() {
  document.documentElement.lang = state.lang === 'zh' ? 'zh-CN' : 'en';
  const lang = $('#lang'), gear = $('#gear');
  lang.textContent = state.lang === 'zh' ? '中' : 'EN';
  lang.title = L('界面语言：中文（点一下换成 English）', 'Language: English (click for 中文)');
  lang.setAttribute('aria-label', lang.title);
  gear.title = L('设置', 'Settings'); gear.setAttribute('aria-label', gear.title);
  $('#project').setAttribute('aria-label', L('切换项目', 'Switch project'));
  $('#dots').setAttribute('aria-label', L('风格', 'Styles'));
  $('#dock').setAttribute('aria-label', L('导出', 'Export'));
}
function wireTop() {
  const pill = $('#project');
  pill.onclick = () => toggleMenu();
  pill.onkeydown = (e) => { if (e.key === 'Enter' || e.key === ' ') toggleMenu(); };
  addEventListener('keydown', (e) => {
    if (e.key === 'Escape') return closeTop();
    if (focusKeys && $('#sheet').hidden && focusKeys(e)) e.preventDefault();
  });
  const t = $('#toast');
  t.onmouseenter = () => clearTimeout(toastTimer);
  t.onmouseleave = () => { if (toastMs > 0) toastTimer = setTimeout(() => (t.hidden = true), 1500); };
  $('#lang').onclick = async () => {
    state.lang = state.lang === 'zh' ? 'en' : 'zh'; store.set('lang', state.lang);
    applyLang(); renderDock(); renderDots(); closeMenu();
    if (state.raw) { rebuildModel(); await paint(); }
    else if (emptyView && !$('#empty').hidden) empty(emptyView, !!state.current);
  };
  $('#gear').onclick = () => settings();
}
function closeMenu() { $('#menu').hidden = true; $('#project').setAttribute('aria-expanded', 'false'); }
function toggleMenu() {
  const menu = $('#menu'), pill = $('#project');
  if (!menu.hidden) return closeMenu();
  pill.setAttribute('aria-expanded', 'true');
  menu.hidden = false;
  menu.innerHTML = `<input id="q" placeholder="${L('搜索项目', 'Search projects')}" autocomplete="off"><div class="list" id="plist" role="listbox" aria-label="${L('项目', 'Projects')}"></div>
    <div class="foot"><button id="add">${L('＋ 添加项目', '+ Add project')}</button><button id="re">↻ ${L('重新分析', 'Re-analyze')}</button></div>`;
  const draw = () => {
    const q = ($('#q') as HTMLInputElement).value.trim().toLowerCase();
    $('#plist').innerHTML = state.projects.filter((p) => !q || `${p.name} ${p.path}`.toLowerCase().includes(q)).map((p) => `
      <div class="item ${p.id === state.current?.id ? 'on' : ''} ${p.exists === false ? 'gone' : ''}" data-id="${esc(p.id)}" role="option" aria-selected="${p.id === state.current?.id}">
        <b>${esc(p.name)}${p.exists === false ? `<em class="tag">${L('目录不在了', 'folder missing')}</em>` : ''}</b><span class="r">${p.lastAnalyzed ? new Date(p.lastAnalyzed * 1000).toLocaleDateString(state.lang === 'zh' ? 'zh-CN' : 'en-US') : L('未分析', 'not analyzed')}</span>
        <span title="${esc(p.path)}">${esc(p.path)}</span><span class="r">${p.evidence ? `${fmt(p.evidence)} ${L('条证据', 'evidence')}` : ''}</span>
        ${p.paths?.length > 1 ? `<span class="paths">${L('还在', 'also at')} ${esc(p.paths.slice(1, 3).join(L('、', ', ')))}${p.paths.length > 3 ? ' …' : ''}</span>` : ''}
      </div>`).join('') || `<div class="item"><span>${L('没有匹配的项目', 'No match')}</span></div>`;
  };
  // ↑ ↓ 选，回车打开（没选就开第一个）
  let kb = -1;
  const items = () => [...$('#plist').querySelectorAll<HTMLElement>('.item[data-id]')];
  const mark = () => items().forEach((x, j) => { x.classList.toggle('kb', j === kb); if (j === kb) x.scrollIntoView({ block: 'nearest' }); });
  draw();
  const q = $('#q') as HTMLInputElement;
  q.oninput = () => { kb = -1; draw(); };
  q.onkeydown = (e) => {
    const n = items().length;
    if (e.key === 'ArrowDown' && n) { kb = (kb + 1) % n; mark(); e.preventDefault(); }
    else if (e.key === 'ArrowUp' && n) { kb = (kb - 1 + n) % n; mark(); e.preventDefault(); }
    else if (e.key === 'Enter' && n) { items()[Math.max(kb, 0)].click(); e.preventDefault(); }
  };
  q.focus();
  $('#plist').onclick = (e) => {
    const it = (e.target as HTMLElement).closest('.item') as HTMLElement | null; if (!it?.dataset.id) return;
    menu.hidden = true; pill.setAttribute('aria-expanded', 'false');
    const p = state.projects.find((x) => x.id === it.dataset.id);
    if (p && p !== state.current) openProject(p);
  };
  const re = $('#re') as HTMLButtonElement;
  if (!state.current || state.current.exists === false || analyzing) { re.disabled = true; re.title = state.current?.exists === false ? L('项目目录已经不在了', 'The project folder is gone') : ''; }
  ($('#add') as HTMLButtonElement).disabled = analyzing;
  $('#add').onclick = () => { closeMenu(); addProject(); };
  re.onclick = () => { closeMenu(); if (state.current) reanalyze(state.current); };
  setTimeout(() => document.addEventListener('click', function off(e) { if (!menu.contains(e.target as Node) && !pill.contains(e.target as Node)) { menu.hidden = true; pill.setAttribute('aria-expanded', 'false'); document.removeEventListener('click', off); } }), 0);
}

// ───────────────────────── 分析：进度、取消 ─────────────────────────
let analyzing = false;
/** 跑一次分析：一直挂着进度提示（秒数、取消），跑完或出错再换成结果 */
async function withProgress<T>(name: string, job: () => Promise<T>): Promise<T> {
  if (analyzing) throw { code: 'busy' };
  analyzing = true;
  const t0 = Date.now();
  const text = () => { const s = Math.round((Date.now() - t0) / 1000); return L(`正在分析 ${name}…${s ? ` ${s} 秒` : ''}`, `Analyzing ${name}…${s ? ` ${s}s` : ''}`); };
  toast(text(), 0, { spin: true, acts: [{ label: L('取消', 'Cancel'), run: () => { toast(L('正在取消…', 'Cancelling…'), 0, { spin: true }); api('cancel_analyze').catch(() => null); } }] });
  const tick = setInterval(() => { const m = document.querySelector('#toast .msg'); if (m && document.querySelector('#toast button')) m.textContent = text(); }, 1000);
  try { return await job(); } finally { clearInterval(tick); analyzing = false; }
}
async function reanalyze(p: any) {
  try {
    const v: any = await withProgress(p.name, () => api('analyze', { projectId: p.id }));
    state.projects = await api('projects');
    const fresh = state.projects.find((x) => x.id === p.id) ?? p;
    // 正看着这个项目：原地换，不清空墙
    if (state.current?.id === p.id && state.raw) {
      const st = readStamp(fresh);
      const raw = await api('credits', { projectId: p.id });
      state.current = fresh; stampOf = await st;
      await swapRaw(raw);
    } else await openProject(fresh);
    done(L('已更新', 'Updated'), v?.warnings);
  } catch (e) { toast(errText(e), 6000); }
}
async function addProject() {
  let dir: string | null;
  try { dir = await api('pick_folder'); } catch (e) { return toast(errText(e), 5000); }
  if (!dir) return;
  const name = dir.split(/[\\/]/).filter(Boolean).pop() ?? dir;
  try {
    const r = await withProgress(name, () => api('add_project', { path: dir }));
    state.projects = await api('projects');
    const p = state.projects.find((x) => x.id === r?.id);
    if (p) await openProject(p);
    done(L(`已添加 ${name}`, `Added ${name}`), r?.result?.warnings);
  } catch (e) { toast(errText(e), 6000); }
}

/** 分析跑完：核心有提醒（某个数据源读不了之类）就在提示里给个「查看」 */
function done(msg: string, warnings?: string[]) {
  if (!warnings?.length) return toast(msg);
  toast(L(`${msg} · ${warnings.length} 条提醒`, `${msg} · ${warnings.length} ${warnings.length > 1 ? 'notes' : 'note'}`), 10000, { acts: [{ label: L('查看', 'View'), run: () => showWarnings(warnings) }] });
}
function showWarnings(list: string[]) {
  const el = $('#sheet');
  el.hidden = false;
  el.innerHTML = `<div class="panel glass">
    <div class="head"><h2>${L('分析提醒', 'Analysis notes')}</h2>${closeBtn()}</div>
    <p>${L('分析完成了，但核心报告了下面这些情况：', 'The analysis finished, and the core reported:')}</p>
    <ul class="warn">${list.map((w) => `<li>${esc(w)}</li>`).join('')}</ul>
  </div>`;
  el.onclick = (e) => { if (e.target === el || (e.target as HTMLElement).closest('[data-act="close"]')) closeSheet(); };
}

function closeSheet() { const el = $('#sheet'); el.hidden = true; el.innerHTML = ''; }
const SIZE_USE: Record<string, [string, string]> = { og: ['链接预览', 'Link preview'], square: ['方图', 'Square'], portrait: ['竖图', 'Portrait'], story: ['竖屏', 'Story'] };
async function settings() {
  const el = $('#sheet');
  el.hidden = false;
  const info = native ? await api('app_info').catch(() => null) : null;
  const sizes = SIZES.filter((s) => s.id !== 'a4').map((s) => `<button data-s="${s.id}" class="${s.id === state.size ? 'on' : ''}" title="${esc(SIZE_USE[s.id] ? L(...SIZE_USE[s.id]) : '')}">${s.w ? `${s.w}×${s.h}` : L('原尺寸', 'Native')}</button>`).join('');
  el.innerHTML = `<div class="panel glass">
    <div class="head"><h2>${L('设置', 'Settings')}</h2>${closeBtn()}</div>
    <h3>${L('导出', 'Export')}</h3>
    ${info ? `<div class="row kv" id="xrow"></div>` : ''}
    <div class="row kv"><span class="k">${L('默认尺寸', 'Default size')}</span><div class="seg" id="defsize">${sizes}</div></div>
    ${info ? `<h3>${L('作品库', 'Library')}</h3><div class="row kv"><span class="k">${L('位置', 'Location')}</span><span class="v path" title="${esc(info.library)}">${esc(short(info.library))}</span><button class="mini" data-x="open-library">${L('打开', 'Open')}</button></div>` : ''}
    <h3>${L('数据源', 'Data sources')}</h3><div id="doc"><p>${L('检查中…', 'Checking…')}</p></div>
    <h3>${L('关于', 'About')}</h3>
    ${info ? `<p class="ver">creditia ${esc(info.version)}</p>` : ''}
    <p>${L('分析在本机完成：读各工具的日志和 git 历史，只保存行的哈希和时间、模型这类信息，不保存代码和 prompt，也不调用任何模型。', 'Analysis runs on this machine: it reads tool logs and git history, keeps only line hashes and metadata such as time and model, never code or prompts, and calls no models.')}</p>
  </div>`;
  const xrow = (x: any) => {
    const r = el.querySelector('#xrow'); if (!r) return;
    r.innerHTML = `<span class="k">${L('保存到', 'Save to')}</span><span class="v path" title="${esc(x.exportDir)}">${esc(short(x.exportDir))}</span><button class="mini" data-x="choose">${L('更改…', 'Change…')}</button>${x.exportDefault ? '' : `<button class="mini" data-x="reset">${L('恢复默认', 'Reset')}</button>`}<button class="mini" data-x="open-exports">${L('打开', 'Open')}</button>`;
  };
  if (info) xrow(info);
  el.onclick = async (e) => {
    const t = e.target as HTMLElement;
    if (e.target === el || t.closest('[data-act="close"]')) return closeSheet();
    const sz = t.closest('#defsize button') as HTMLElement | null;
    if (sz) {
      state.size = sz.dataset.s!; store.set('size', state.size);
      el.querySelectorAll('#defsize button').forEach((b) => b.classList.toggle('on', b === sz));
      return renderDock();
    }
    const x = (t.closest('[data-x]') as HTMLElement | null)?.dataset.x;
    try {
      if (x === 'choose') { if (await api('choose_export_dir')) xrow(await api('app_info')); }
      else if (x === 'reset') { await api('reset_export_dir'); xrow(await api('app_info')); }
      else if (x === 'open-exports') await api('open_folder', { which: 'exports' });
      else if (x === 'open-library') await api('open_folder', { which: 'library' });
    } catch (err) { toast(errText(err), 5000); }
  };
  const doc = await api('doctor').catch(() => []);
  const detail = (d: any) => !d.ok ? L('没有找到', 'Not found')
    : d.records != null ? L(`${fmt(d.records)} 条记录`, `${fmt(d.records)} records`)
    : d.entries != null ? L(`${fmt(d.entries)} 个条目`, `${fmt(d.entries)} entries`)
    : L('已就绪', 'Ready');
  $('#doc').innerHTML = doc.length ? doc.map((d: any) => `<div class="row" title="${esc(d.path ?? '')}"><i class="dot ${d.ok ? 'ok' : ''}"></i><span class="k">${esc(nameOf(d.id, state.lang))}</span><span class="v">${esc(detail(d))}</span></div>`).join('') : `<p>${L('没有可用的数据源', 'No data sources found')}</p>`;
}

boot();
