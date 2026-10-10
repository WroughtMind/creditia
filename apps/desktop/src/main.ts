// creditia App：一个主界面——左边选项目，中间是大图，右边三步：选图 → 选风格 → 保存。
// 渲染和 CLI 共用同一套 forms（satori → SVG，resvg-wasm → PNG）。
import { THEMES, themeById } from '../../../packages/render/src/themes.ts';
import { buildModel, fmt, pct, type Model } from '../../../packages/render/src/data.ts';
import { FORMS, GROUPS, type Ctx } from '../../../packages/render/src/registry.ts';
import { creditsBlock, mergeCreditsMd, unitLead } from '../../../packages/render/src/text.ts';
import { famColor } from '../../../packages/render/src/brand.ts';
import { renderSvg, renderSized, toPng } from './preview.ts';
import { THEME_ZH, VARIANT_ZH, SIZES, revealLabel } from './labels.ts';

// ───────────────────────── Tauri 桥 ─────────────────────────
const native = (window as any).__TAURI__?.core?.invoke as ((cmd: string, args?: any) => Promise<any>) | undefined;
async function api(cmd: string, args: any = {}): Promise<any> {
  if (native) return native(cmd, args);
  switch (cmd) {
    case 'projects': return (await fetch('/__dev/projects')).json();
    case 'credits': return (await fetch(`/__dev/credits?id=${encodeURIComponent(args.projectId)}`)).json();
    case 'config_of': return (await fetch('/__dev/config')).json();
    case 'doctor': return (await fetch('/__dev/doctor')).json();
    case 'set_display_director': return null;
    case 'reveal': case 'open_path': toast('浏览器预览里打不开文件夹，要在 App 里用'); return null;
    case 'save_file': {
      const a = document.createElement('a');
      a.href = URL.createObjectURL(new Blob([b64bytes(args.bytes)], { type: args.mime ?? 'application/octet-stream' }));
      a.download = args.name; a.click(); URL.revokeObjectURL(a.href);
      return `下载文件夹/${args.name}`;
    }
    default: throw new Error('浏览器预览里没有这个功能：' + cmd);
  }
}

// ───────────────────────── 状态 ─────────────────────────
type View = 'make' | 'detail';
const $ = <T extends HTMLElement = HTMLElement>(s: string) => document.querySelector(s) as T;
const ORDER = GROUPS.flatMap(([, ids]) => ids).filter((id) => id !== 'blame'); // blame 要指定某一行，只在 CLI 里用
const state = {
  projects: [] as any[], current: null as any, config: {} as any,
  raw: null as any, model: null as Model | null, library: null as Model[] | null,
  view: 'make' as View,
  theme: (localStorage.creditiaTheme as string) || 'cinema',
  form: (localStorage.creditiaForm as string) || 'tier',
  variant: undefined as string | undefined, size: 'native',
  svg: '', token: 0, thumbToken: 0,
};
if (!ORDER.includes(state.form)) state.form = 'tier';
const theme = () => themeById(state.theme) ?? THEMES[0];
const esc = (s: unknown) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!);
function toast(msg: string) { const el = $('#toast'); el.textContent = msg; el.hidden = false; clearTimeout((toast as any)._t); (toast as any)._t = setTimeout(() => (el.hidden = true), 2800); }
async function copy(text: string, ok = '已复制') { try { await navigator.clipboard.writeText(text); toast(ok); } catch { toast('没能复制，系统不让访问剪贴板'); } }
const b64 = (bytes: Uint8Array) => { let s = ''; for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000)); return btoa(s); };
const b64bytes = (s: string) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
const analyzeCmd = (path = '你的项目文件夹') => `cd "${path}" && creditia analyze`;

// ───────────────────────── 启动 ─────────────────────────
async function boot() {
  $('#gear').onclick = settingsSheet;
  $('#proj').onclick = (e) => projectMenu(e.currentTarget as HTMLElement);
  $('#nav').onclick = (e) => {
    const b = (e.target as HTMLElement).closest('button[data-view]') as HTMLElement | null; if (!b) return;
    state.view = b.dataset.view as View; renderView();
  };
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape') closePop(); });
  try { state.projects = await api('projects'); } catch (e: any) { return firstRun(`没能读取项目列表：${e?.message ?? e}`); }
  if (state.projects.length) openProject(state.projects[0]); else firstRun();
}

// ───────────────────────── 弹出菜单（项目、图、风格、版式、尺寸、更多） ─────────────────────────
function closePop() { const p = $('#pop'); p.hidden = true; p.innerHTML = ''; document.querySelectorAll('.open').forEach((x) => x.classList.remove('open')); }
function openPop(anchor: HTMLElement, html: string, opts: { up?: boolean; width?: number } = {}) {
  const was = anchor.classList.contains('open');
  closePop(); if (was) return null;
  anchor.classList.add('open');
  const p = $('#pop'); p.hidden = false;
  p.innerHTML = `<div class="pop-back"></div><div class="menu" style="${opts.width ? `width:${opts.width}px;` : ''}">${html}</div>`;
  const m = p.querySelector('.menu') as HTMLElement; const r = anchor.getBoundingClientRect();
  const w = m.offsetWidth;
  m.style.left = `${Math.max(12, Math.min(window.innerWidth - w - 12, r.left + r.width / 2 - w / 2))}px`;
  if (opts.up) m.style.bottom = `${window.innerHeight - r.top + 8}px`; else m.style.top = `${r.bottom + 8}px`;
  (p.querySelector('.pop-back') as HTMLElement).onclick = closePop;
  return m;
}
function projectMenu(anchor: HTMLElement) {
  const m = openPop(anchor, `
    ${state.model ? `<p class="menu-sum">${esc(summary(state.model))}</p>` : ''}
    ${state.projects.length > 6 ? '<input class="menu-q" placeholder="找项目">' : ''}
    <div class="menu-list" id="plist"></div>
    <div class="menu-sep"></div>
    <button class="menu-item" id="padd">添加项目…</button>`, { width: 340 });
  if (!m) return;
  const list = () => {
    const q = (m.querySelector('.menu-q') as HTMLInputElement | null)?.value.trim().toLowerCase() ?? '';
    m.querySelector('#plist')!.innerHTML = state.projects.filter((p) => !q || String(p.name).toLowerCase().includes(q)).map((p) => {
      const when = p.lastAnalyzed ? new Date(p.lastAnalyzed * 1000).toLocaleDateString('zh-CN', { month: 'long', day: 'numeric' }) + '分析' : '还没分析';
      return `<button class="menu-item ${p.id === state.current?.id ? 'on' : ''}" data-pid="${esc(p.id)}" title="${esc(p.path)}"><span>${esc(p.name)}</span><span class="dim">${when}</span></button>`;
    }).join('');
  };
  list();
  m.querySelector('.menu-q')?.addEventListener('input', list);
  m.onclick = (e) => {
    const b = (e.target as HTMLElement).closest('button') as HTMLElement | null; if (!b) return;
    if (b.id === 'padd') { closePop(); return addSheet(); }
    const p = state.projects.find((x) => x.id === b.dataset.pid); if (p) { closePop(); openProject(p); }
  };
}

/** 第一次打开、还没有任何项目时：说清楚这是什么、下一步做什么 */
function firstRun(err?: string) {
  $('#name').textContent = '还没有项目';
  $('#nav').hidden = true;
  $('#view').innerHTML = `
  <div class="welcome">
    <h1>creditia</h1>
    <p class="lead">看你项目的 git 提交和本机 AI 编程工具的记录，算出每段代码是谁写的，做成能分享的图。</p>
    ${err ? `<p class="warn">${esc(err)}</p>` : ''}
    <ol class="steps">
      <li><b>分析一个项目</b><span>在终端里进入项目文件夹，运行下面这行。只在本机读，不上传任何东西。</span>
        <div class="row"><code>${esc(analyzeCmd())}</code><button class="btn" id="cp-first">复制</button></div></li>
      <li><b>回到这里</b><span>分析完的项目会出现在左边。</span></li>
      <li><b>挑一张图保存</b><span>排行榜、片尾字幕、小票……选一个风格，点「保存图片」。</span></li>
    </ol>
    <button class="btn primary" id="reload">我已经分析好了，刷新</button>
  </div>`;
  $('#cp-first').onclick = () => copy(analyzeCmd(), '命令已复制');
  $('#reload').onclick = () => location.reload();
}

/** 一句话说清这个项目：谁写的、多少 */
function summary(m: Model) {
  const agents = m.rows.filter((r) => r.key !== 'human' && r.key !== 'unattributed').slice(0, 3).map((r) => `${r.name} ${pct(r.pct)}`);
  const bits = [
    `${fmt(m.commits)} 次提交`,
    m.spanDays ? `做了 ${m.spanDays} 天` : '',
    `写了 ${fmt(m.linesAdded)} 行，现在还在 ${fmt(m.linesSurviving)} 行`,
  ].filter(Boolean).join(' · ');
  return `${bits}${agents.length ? `。主要由 ${agents.join('、')} 写成` : ''}${m.director ? `，${m.director} 主导` : ''}。`;
}

async function openProject(p: any) {
  state.current = p; state.raw = null; state.model = null; state.variant = undefined;
  $('#name').textContent = p.name;
  try {
    state.raw = await api('credits', { projectId: p.id });
    state.config = await api('config_of', { projectId: p.id }).catch(() => ({}));
  } catch (e: any) {
    $('#view').innerHTML = `<div class="empty"><p>这个项目还没有分析结果。</p><div class="row"><code>${esc(analyzeCmd(p.path))}</code><button class="btn" id="cp-an">复制</button></div><p class="meta">${esc(e?.message ?? e)}</p></div>`;
    $('#cp-an').onclick = () => copy(analyzeCmd(p.path), '命令已复制');
    return;
  }
  state.model = buildModel(state.raw, { director: !!state.config.display?.director, lang: 'zh' });
  $('#name').textContent = state.model.project || p.name;
  $('#proj').title = summary(state.model);
  $('#nav').hidden = false;
  renderView();
}

function renderView() {
  $('#nav').querySelectorAll('button[data-view]').forEach((b) => b.classList.toggle('on', (b as HTMLElement).dataset.view === state.view));
  const v = $('#view'); v.innerHTML = ''; v.scrollTop = 0;
  if (!state.model) return;
  if (state.view === 'detail') viewDetail(v); else viewMake(v);
}

// ───────────────────────── 主界面：做一张图 ─────────────────────────
/** 尺寸字符串里的宽高比；自适应的先按 4:3 占位，渲染完再按实际比例改 */
function aspectOf(id: string) {
  const m = /^(\d+)×(\d+)$/.exec(FORMS[id].size);
  return m ? `${m[1]} / ${m[2]}` : '4 / 3';
}
const curVariant = () => state.variant ?? FORMS[state.form].def?.(theme());

const chev = '<svg width="9" height="9" viewBox="0 0 10 10"><path d="M2 3.5 5 6.5 8 3.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>';
function viewMake(v: HTMLElement) {
  v.innerHTML = `
  <div class="stage" id="stage"><div class="loading">正在画…</div></div>
  <div class="bar" id="bar">
    <button class="pick" id="b-form"></button>
    <button class="pick" id="b-theme"></button>
    <button class="pick" id="b-var"></button>
    <button class="pick" id="b-size"></button>
    <label class="pick toggle" id="b-dir" title="在图上写明谁是主导者"><input type="checkbox" id="dir"><span>署主导者</span></label>
    <span class="bar-sep"></span>
    <button class="btn primary" id="sv-png">保存图片</button>
    <button class="icon" id="b-more" title="更多导出方式"><svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor"><circle cx="5" cy="12" r="1.8"/><circle cx="12" cy="12" r="1.8"/><circle cx="19" cy="12" r="1.8"/></svg></button>
  </div>`;
  $('#b-form').onclick = (e) => formMenu(e.currentTarget as HTMLElement);
  $('#b-theme').onclick = (e) => themeMenu(e.currentTarget as HTMLElement);
  $('#b-var').onclick = (e) => listMenu(e.currentTarget as HTMLElement, (FORMS[state.form].variants ?? []).map((x) => [x, VARIANT_ZH[state.form]?.[x] ?? x]), curVariant(), (x) => { state.variant = x; });
  $('#b-size').onclick = (e) => listMenu(e.currentTarget as HTMLElement, SIZES, state.size, (x) => { state.size = x; });
  $('#b-more').onclick = (e) => moreMenu(e.currentTarget as HTMLElement);
  $('#sv-png').onclick = savePng;
  ($('#dir') as HTMLInputElement).addEventListener('change', async (e) => {
    const on = (e.target as HTMLInputElement).checked;
    state.config.display = { ...(state.config.display ?? {}), director: on };
    state.model = buildModel(state.raw, { director: on, lang: 'zh' });
    await api('set_display_director', { projectId: state.current.id, on }).catch(() => {});
    paintFocus();
  });
  renderOptions(); paintFocus();
}

/** 底部工具条上的按钮文字跟着当前选择走；没有版式的图不显示版式 */
function renderOptions() {
  const f = FORMS[state.form]; const v = curVariant();
  $('#b-form').innerHTML = `<span class="k">图</span>${esc(f.name)}${chev}`;
  $('#b-theme').innerHTML = `<span class="k">风格</span><span class="sw" style="background:${theme().bg}"><i style="background:${theme().accent}"></i></span>${esc(THEME_ZH[state.theme] ?? state.theme)}${chev}`;
  const bv = $('#b-var'); bv.hidden = !f.variants?.length;
  if (v) bv.innerHTML = `<span class="k">版式</span>${esc(VARIANT_ZH[state.form]?.[v] ?? v)}${chev}`;
  $('#b-size').innerHTML = `<span class="k">尺寸</span>${esc((SIZES.find(([k]) => k === state.size)?.[1] ?? '').replace(/ \d.*/, ''))}${chev}`;
  $('#b-dir').hidden = state.form !== 'tier';
  ($('#dir') as HTMLInputElement).checked = !!state.config.display?.director;
}

function listMenu(anchor: HTMLElement, items: [string, string][], cur: string | undefined, set: (x: string) => void) {
  const m = openPop(anchor, items.map(([k, n]) => `<button class="menu-item ${k === cur ? 'on' : ''}" data-k="${esc(k)}"><span>${esc(n)}</span></button>`).join(''), { up: true, width: 220 });
  if (!m) return;
  m.onclick = (e) => { const b = (e.target as HTMLElement).closest('[data-k]') as HTMLElement | null; if (!b) return; set(b.dataset.k!); closePop(); renderOptions(); paintFocus(); };
}
function themeMenu(anchor: HTMLElement) {
  const m = openPop(anchor, `<div class="theme-grid">${THEMES.map((t) => `<button class="menu-item ${t.id === state.theme ? 'on' : ''}" data-k="${t.id}"><span class="sw" style="background:${t.bg}"><i style="background:${t.accent}"></i></span><span>${esc(THEME_ZH[t.id] ?? t.name)}</span></button>`).join('')}</div>`, { up: true, width: 340 });
  if (!m) return;
  m.onclick = (e) => {
    const b = (e.target as HTMLElement).closest('[data-k]') as HTMLElement | null; if (!b) return;
    state.theme = b.dataset.k!; localStorage.creditiaTheme = state.theme; closePop(); renderOptions(); paintFocus();
  };
}
function formMenu(anchor: HTMLElement) {
  const m = openPop(anchor, GROUPS.map(([g, ids]) => {
    const list = ids.filter((id) => ORDER.includes(id));
    return list.length ? `<h4>${esc(g)}</h4><div class="tgrid">${list.map((id) => `
      <button class="thumb ${id === state.form ? 'on' : ''}" data-form="${id}" title="${esc(FORMS[id].q)}">
        <span class="tstage" style="aspect-ratio:${aspectOf(id)}" data-tstage="${id}"></span>
        <span class="tname">${esc(FORMS[id].name)}</span><span class="tq">${esc(FORMS[id].q)}</span>
      </button>`).join('')}</div>` : '';
  }).join(''), { up: true, width: Math.min(760, window.innerWidth - 24) });
  if (!m) return;
  m.classList.add('forms');
  m.onclick = (e) => {
    const b = (e.target as HTMLElement).closest('[data-form]') as HTMLElement | null; if (!b) return;
    state.form = b.dataset.form!; localStorage.creditiaForm = state.form; state.variant = undefined;
    closePop(); renderOptions(); paintFocus();
  };
  paintThumbs(m);
}
function moreMenu(anchor: HTMLElement) {
  const items: [string, string][] = [['png', '复制图片'], ['svg', '复制 SVG'], ...(state.form === 'tier' ? [['embed', '复制网页嵌入代码'] as [string, string]] : []), ['md', '复制 CREDITS.md']];
  const m = openPop(anchor, items.map(([k, n]) => `<button class="menu-item" data-k="${k}"><span>${n}</span></button>`).join(''), { up: true, width: 200 });
  if (!m) return;
  m.onclick = (e) => {
    const k = ((e.target as HTMLElement).closest('[data-k]') as HTMLElement | null)?.dataset.k; if (!k) return;
    closePop();
    if (k === 'png') copyPng(); else if (k === 'svg') copy(state.svg, 'SVG 已复制'); else if (k === 'embed') copyEmbed();
    else copy(mergeCreditsMd(null, creditsBlock(state.model!)), 'CREDITS.md 已复制');
  };
}

/** profile 要全部项目的模型；其余只要语言和版式 */
async function ctxOf(id: string, variant?: string): Promise<Ctx> {
  const ctx: Ctx = { lang: 'zh', variant };
  if (id !== 'profile') return ctx;
  if (!state.library) {
    const all = await Promise.all(state.projects.map(async (p) => { try { return buildModel(await api('credits', { projectId: p.id }), { director: false, lang: 'zh' }); } catch { return null; } }));
    state.library = all.filter(Boolean) as Model[];
  }
  ctx.projects = state.library.length ? state.library : [state.model!];
  ctx.person = state.raw?.directors?.[0]?.name ?? '';
  return ctx;
}

async function paintFocus() {
  const out = $('#stage'); if (!out) return;
  const token = ++state.token;
  const id = state.form;
  out.classList.add('busy');
  let svg: string | null = null;
  try {
    const ctx = await ctxOf(id, state.variant);
    if (state.size === 'native') svg = await renderSvg(id, state.model!, theme(), ctx);
    else { const [w, h] = state.size.split('x').map(Number); svg = await renderSized(id, state.model!, theme(), w, h, ctx); }
  } catch (e) { console.error(e); }
  if (token !== state.token) return;
  out.classList.remove('busy');
  state.svg = svg ?? '';
  out.innerHTML = svg ?? `<div class="empty-card"><b>这个项目的数据画不出这张图</b><span>${esc(whyEmpty(id))}</span></div>`;
  ($('#sv-png') as HTMLButtonElement).disabled = !svg;
}
function whyEmpty(id: string) {
  if (id === 'profile') return '需要至少一个分析过的项目。';
  if (id === 'lanes') return '需要本机有 agent 会话记录。';
  if (id === 'versions') return '需要项目打过版本 tag。';
  return '数据太少，换一张图试试。';
}

/** 缩略图：滚到眼前才画，大小按每张图的真实比例 */
let io: IntersectionObserver | null = null;
function paintThumbs(root: HTMLElement) {
  const token = ++state.thumbToken;
  io?.disconnect();
  if (!state.model) return;
  const queue: string[] = []; let busy = 0;
  const pump = async () => {
    while (busy < 2 && queue.length) {
      const id = queue.shift()!; busy++;
      (async () => {
        let svg: string | null = null;
        try { svg = await renderSvg(id, state.model!, theme(), await ctxOf(id)); } catch { svg = null; }
        busy--;
        if (token !== state.thumbToken) return;
        const el = root.querySelector(`[data-tstage="${id}"]`) as HTMLElement | null;
        if (el) {
          el.innerHTML = svg ?? '<span class="ph">—</span>';
          const vb = svg && /viewBox="0 0 ([\d.]+) ([\d.]+)"/.exec(svg);
          if (vb) el.style.aspectRatio = `${vb[1]} / ${vb[2]}`;
        }
        pump();
      })();
    }
  };
  io = new IntersectionObserver((ents) => {
    for (const e of ents) if (e.isIntersecting) { io!.unobserve(e.target); queue.push((e.target as HTMLElement).dataset.tstage!); }
    pump();
  }, { root, rootMargin: '200px' });
  root.querySelectorAll('[data-tstage]').forEach((el) => io!.observe(el));
}

const fileName = () => `${state.model!.project || 'credits'}-${state.form}${state.variant ? '-' + state.variant : ''}.png`;
async function savePng() {
  if (!state.svg) return;
  const png = await toPng(state.svg);
  const path = await api('save_file', { name: fileName(), bytes: b64(png), mime: 'image/png' });
  toast(`已保存到 ${path}`);
}
async function copyPng() {
  try {
    const png = await toPng(state.svg);
    await navigator.clipboard.write([new ClipboardItem({ 'image/png': new Blob([png as BlobPart], { type: 'image/png' }) })]);
    toast('图片已复制，可以直接粘贴');
  } catch { toast('这里不能直接复制图片，请用「保存图片」'); }
}
async function copyEmbed() {
  const director = !!state.config.display?.director;
  const s = native
    ? await api('embed_snippet', { projectId: state.current.id, form: state.form, theme: state.theme, director })
    : `<script type="module" src="https://cdn.jsdelivr.net/npm/@creditia/embed@0/dist/creditia-card.js"><\/script>\n<creditia-card src=".creditia/credits.json" form="${state.form}" theme="${state.theme}"${director ? ' director' : ''}></creditia-card>`;
  copy(s, '嵌入代码已复制，贴到网页 HTML 里即可');
}

// ───────────────────────── 逐条明细 ─────────────────────────
function viewDetail(v: HTMLElement) {
  const raw = state.raw, m = state.model!;
  const units: any[] = raw.units ?? [];
  const groups: { title: string; date?: string; units: any[] }[] = [];
  const unrel = units.filter((u) => !u.version);
  if (unrel.length) groups.push({ title: raw.versions?.length ? '还没发版的' : '全部', units: unrel });
  for (const ver of [...(raw.versions ?? [])].reverse()) { const us = units.filter((u) => u.version === ver.tag); if (us.length) groups.push({ title: ver.tag, date: ver.date, units: us }); }
  const uid = (id: string) => (id.startsWith('pr:') ? `PR #${id.slice(3)}` : `提交 ${id.slice(2, 9)}`);
  const legend = new Map<string, string>();
  const shareBar = (u: any) => {
    const by = new Map<string, number>();
    for (const s of u.shares ?? []) { const c = (raw.contributors ?? []).find((x: any) => x.id === s.contributor); const k = c?.harness ?? (s.contributor === 'unattributed' ? 'unattributed' : 'human'); by.set(k, (by.get(k) ?? 0) + s.share); }
    return `<div class="ubar">${[...by.entries()].sort((a, b) => b[1] - a[1]).map(([k, s]) => {
      const name = k === 'unattributed' ? '认不出来' : k === 'human' ? '人' : (m.rows.find((r) => r.key === k)?.name ?? k);
      const bg = k === 'unattributed' ? 'var(--unknown)' : famColor(k, theme());
      legend.set(name, bg);
      return `<i style="width:${(s * 100).toFixed(1)}%;background:${bg}" title="${esc(name)} ${Math.round(s * 100)}%"></i>`;
    }).join('')}</div>`;
  };
  const prs = units.filter((u) => String(u.id).startsWith('pr:')).length;
  const body = groups.map((g) => `<div class="ver"><h3>${esc(g.title)}${g.date ? ` <span class="dim">${g.date}</span>` : ''}<span class="count">${g.units.length} 条</span></h3>
      ${[...g.units].sort((a, b) => String(b.date).localeCompare(String(a.date))).map((u) => `
      <div class="unit">
        <div class="uinfo"><span class="uid mono">${esc(uid(String(u.id)))}</span><span class="utitle">${esc(String(u.title ?? '').replace(/\s*\(#\d+\)\s*$/, '').replace(/\s*Creditia-Harness:.*$/, ''))}</span><span class="ulead">${esc(unitLead(raw, u))}</span><span class="udate mono">${u.date}</span></div>
        ${shareBar(u)}
      </div>`).join('')}
    </div>`).join('');
  v.innerHTML = `
  <div class="detail">
    <div class="phead">
      <div><h2>每个 PR / 提交是谁写的</h2><p class="meta">${prs ? `${prs} 个 PR，` : ''}${units.length - prs} 个直接提交。色条是这一条里各方写的代码占比。</p></div>
      <div class="grow"></div>
      <button class="btn" id="cp-md" title="可以贴进仓库根目录的 CREDITS.md">复制 CREDITS.md</button>
      <button class="btn" id="finder">${revealLabel()}</button>
    </div>
    <div class="legend" id="legend"></div>
    ${groups.length ? body : '<div class="empty">还没有记录。</div>'}
  </div>`;
  $('#legend').innerHTML = [...legend.entries()].map(([n, c]) => `<span><i style="background:${c}"></i>${esc(n)}</span>`).join('');
  $('#cp-md').onclick = () => copy(mergeCreditsMd(null, creditsBlock(m)), 'CREDITS.md 已复制');
  $('#finder').onclick = () => api('reveal', { path: state.current.path });
}

// ───────────────────────── 浮层：添加项目 / 设置 ─────────────────────────
function sheet(html: string) {
  const el = $('#sheet'); el.hidden = false;
  el.innerHTML = `<div class="sheet-back" id="sheet-back"></div><div class="sheet" role="dialog">${html}<button class="x" id="sheet-x" title="关闭">×</button></div>`;
  const close = () => { el.hidden = true; el.innerHTML = ''; document.removeEventListener('keydown', onKey); };
  const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') close(); };
  document.addEventListener('keydown', onKey);
  $('#sheet-back').onclick = close; $('#sheet-x').onclick = close;
}
function addSheet() {
  sheet(`<h2>添加项目</h2>
    <p class="meta">在终端里进入项目文件夹运行这行，分析完会自动出现在左边。之后有新提交，再跑一次就会更新。</p>
    <div class="row"><code id="cmd">${esc(analyzeCmd())}</code><button class="btn" id="cp-cmd">复制</button></div>`);
  $('#cp-cmd').onclick = () => copy(analyzeCmd(), '命令已复制');
}
async function settingsSheet() {
  sheet(`<h2>设置与数据来源</h2>
    <h3>从哪里认出 AI 写的代码</h3>
    <p class="meta">creditia 只读本机这些工具的记录，不上传，也不保存你的代码和提示词。</p>
    <div id="doc"><div class="loading">检查中…</div></div>
    <h3>项目数据存在哪</h3><div class="row"><code id="libpath">…</code><button class="btn" id="lib-open">打开</button></div>
    ${state.current ? `<h3>更新当前项目</h3><div class="row"><code id="cmd">${esc(analyzeCmd(state.current.path))}</code><button class="btn" id="cp-cmd">复制</button></div>` : ''}
    <p class="meta foot">creditia 0.1.0</p>`);
  ($('#cp-cmd') as HTMLElement | null)?.addEventListener('click', () => copy(analyzeCmd(state.current.path), '命令已复制'));
  const lp = await api('library_path').catch(() => '');
  $('#libpath').textContent = lp || '浏览器预览里没有';
  $('#lib-open').onclick = () => (lp ? api('open_path', { path: lp }) : toast('浏览器预览里没有'));
  const doc: any[] = await api('doctor').catch(() => []);
  $('#doc').innerHTML = doc.length
    ? doc.map((d) => `<div class="src"><i class="dot ${d.ok ? 'ok' : ''}"></i><b>${esc(d.name)}</b><span class="path">${esc(d.path)}</span><span class="q">${d.ok ? '能读到' : '没找到'}</span></div>`).join('')
    : '<div class="meta">本机没找到任何 AI 编程工具的记录，只能按 git 提交信息推断。</div>';
}

boot();
