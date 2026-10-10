// creditia App：本机作品库 → 图库 → 单张导出。渲染和 CLI 共用同一套 forms（satori → SVG，resvg-wasm → PNG）。
import { THEMES, themeById } from '../../../packages/render/src/themes.ts';
import { buildModel, fmt, compact, type Model } from '../../../packages/render/src/data.ts';
import { FORMS, GROUPS, type Ctx } from '../../../packages/render/src/registry.ts';
import { creditsBlock, mergeCreditsMd, unitLead } from '../../../packages/render/src/text.ts';
import { famColor } from '../../../packages/render/src/brand.ts';
import { renderSvg, renderSized, toPng } from './preview.ts';

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
    case 'reveal': case 'open_path': toast('这一步要在 App 里运行'); return null;
    case 'save_file': {
      const a = document.createElement('a');
      a.href = URL.createObjectURL(new Blob([b64bytes(args.bytes)], { type: args.mime ?? 'application/octet-stream' }));
      a.download = args.name; a.click(); URL.revokeObjectURL(a.href);
      return `~/Downloads/${args.name}`;
    }
    default: throw new Error('浏览器调试模式里没有 ' + cmd);
  }
}

// ───────────────────────── 状态 ─────────────────────────
type View = 'gallery' | 'focus' | 'archive';
const $ = <T extends HTMLElement = HTMLElement>(s: string) => document.querySelector(s) as T;
const SIZES = ['原生', '1200×630', '1080×1350', '1080×1920', '1080×1080'];
const state = {
  projects: [] as any[], current: null as any, config: {} as any,
  raw: null as any, model: null as Model | null,
  view: 'gallery' as View, library: null as Model[] | null, theme: (localStorage.creditiaTheme as string) || 'cinema',
  focus: null as string | null, variant: undefined as string | undefined, size: '原生',
  svg: '', doctor: [] as any[], token: 0,
};
const theme = () => themeById(state.theme) ?? THEMES[0];
const esc = (s: unknown) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!);
function toast(msg: string) { const el = $('#toast'); el.textContent = msg; el.hidden = false; clearTimeout((toast as any)._t); (toast as any)._t = setTimeout(() => (el.hidden = true), 2600); }
async function copy(text: string, ok = '已复制') { try { await navigator.clipboard.writeText(text); toast(ok); } catch { toast('复制失败'); } }
const b64 = (bytes: Uint8Array) => { let s = ''; for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000)); return btoa(s); };
const b64bytes = (s: string) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));

// ───────────────────────── 启动 ─────────────────────────
async function boot() {
  wireNav();
  try { state.projects = await api('projects'); } catch (e: any) { $('#lib').textContent = String(e?.message ?? e); return; }
  renderProjects();
  $('#lib').textContent = state.projects.length ? `${state.projects.length} 个项目` : '作品库为空';
  if (state.projects.length) openProject(state.projects[0]);
  else showEmpty('作品库为空');
  $('#q').addEventListener('input', renderProjects);
}
function renderProjects() {
  const q = ($('#q') as HTMLInputElement).value.trim().toLowerCase();
  const ul = $('#projects'); ul.innerHTML = '';
  for (const p of state.projects.filter((p) => !q || String(p.name).toLowerCase().includes(q))) {
    const li = document.createElement('li');
    li.className = p.id === state.current?.id ? 'on' : '';
    li.innerHTML = `<b>${esc(p.name)}</b><span>${p.lastAnalyzed ? new Date(p.lastAnalyzed * 1000).toLocaleDateString('zh-CN') : '—'} · ${fmt(p.evidence)} 条证据</span>`;
    li.onclick = () => openProject(p);
    ul.append(li);
  }
}
async function openProject(p: any) {
  state.current = p; state.raw = null; state.model = null; state.variant = undefined; state.focus = null; state.view = 'gallery';
  $('#name').textContent = p.name;
  $('#sub').textContent = `${p.path} · 读取中…`;
  renderProjects();
  try {
    state.raw = await api('credits', { projectId: p.id });
    state.config = await api('config_of', { projectId: p.id }).catch(() => ({}));
  } catch (e: any) { showEmpty(String(e?.message ?? e)); return; }
  state.model = buildModel(state.raw, { director: !!state.config.display?.director, lang: 'zh' });
  $('#name').textContent = state.model.project || p.name;
  $('#sub').textContent = p.path;
  renderView();
}
function showEmpty(msg: string) { $('#nav').innerHTML = ''; $('#view').innerHTML = `<div class="empty">${esc(msg)}</div>`; }

// ───────────────────────── 顶部导航 ─────────────────────────
const NAV: [View, string][] = [['gallery', '图库'], ['archive', '档案']];
function wireNav() {
  const el = $('#nav');
  el.innerHTML = NAV.map(([id, n]) => `<button data-view="${id}">${n}</button>`).join('') + '<button class="gear" id="gear" title="设置"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg></button>';
  el.onclick = (e) => {
    const b = (e.target as HTMLElement).closest('button'); if (!b) return;
    if (b.id === 'gear') return settingsSheet();
    state.view = (b as any).dataset.view; renderView();
  };
}
function renderView() {
  $('#nav').querySelectorAll('button[data-view]').forEach((b) => b.classList.toggle('on', (b as any).dataset.view === state.view));
  const v = $('#view'); v.innerHTML = ''; v.scrollTop = 0;
  if (!state.model) return;
  if (state.view === 'focus') viewFocus(v);
  else if (state.view === 'archive') viewArchive(v);
  else viewGallery(v);
}

// ───────────────────────── 图库 ─────────────────────────
function viewGallery(v: HTMLElement) {
  const m = state.model!;
  const forms = (ids: string[]) => ids.filter((id) => id !== 'blame');
  const kpi: [string, string][] = [['提交', fmt(m.commits)], ['项', fmt(m.units)], ['新增行', fmt(m.linesAdded)], ['留下', fmt(m.linesSurviving)], ['会话', m.sessions ? fmt(m.sessions) : '—'], ['Token', m.tokens ? compact(m.tokens) : '—'], ['工期', m.spanDays ? `${m.spanDays} 天` : '—']];
  v.innerHTML = `
    <div class="strip">${kpi.map(([k, x]) => `<div class="cell"><b>${x}</b><span>${k}</span></div>`).join('')}</div>
    ${GROUPS.map(([g, ids]) => `<div class="sec"><h2>${g}</h2><div class="grid">${forms(ids).map((id) => `
      <button class="card" data-form="${id}">
        <div class="stage" data-stage="${id}"><span class="ph">…</span></div>
        <div class="cap"><b>${FORMS[id].name}</b><span>${FORMS[id].variants ? FORMS[id].variants!.length + ' 版式' : FORMS[id].size}</span></div>
      </button>`).join('')}</div></div>`).join('')}`;
  v.querySelectorAll('[data-form]').forEach((b) => ((b as HTMLElement).onclick = () => openFocus((b as any).dataset.form)));
  paintGrid(v);
}
/** profile 要全作品库的模型；其余只要语言和版式 */
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
function openFocus(id: string) { state.focus = id; state.variant = undefined; state.view = 'focus'; renderView(); }
async function paintGrid(v: HTMLElement) {
  const token = ++state.token;
  const ids = GROUPS.flatMap(([, x]) => x).filter((id) => id !== 'blame');
  let i = 0;
  const worker = async () => {
    while (i < ids.length) {
      const id = ids[i++];
      let svg: string | null = null;
      try { svg = await renderSvg(id, state.model!, theme(), await ctxOf(id)); } catch { svg = null; }
      if (token !== state.token) return;
      const el = v.querySelector(`[data-stage="${id}"]`);
      if (el) el.innerHTML = svg ?? '<span class="ph">—</span>';
    }
  };
  await Promise.all([worker(), worker(), worker()]);
}

// ───────────────────────── 单张 ─────────────────────────
function viewFocus(v: HTMLElement) {
  const id = state.focus!; const f = FORMS[id];
  const variants = f.variants ?? [];
  const cur = state.variant ?? f.def?.(theme());
  v.innerHTML = `
  <div class="focus">
    <div class="stage big">
      <button class="back" id="back">← 图库</button>
      <div class="out" id="ex-out"><div class="loading">渲染中…</div></div>
    </div>
    <aside class="rail">
      <h2>${f.name}<span>${f.size}</span></h2>
      <h3>风格</h3><div class="swatches" id="themes"></div>
      ${variants.length ? `<h3>版式</h3><div class="chips">${variants.map((x) => `<button class="chip ${x === cur ? 'on' : ''}" data-var="${x}">${x}</button>`).join('')}</div>` : ''}
      <h3>尺寸</h3><select id="size">${SIZES.map((s) => `<option ${s === state.size ? 'selected' : ''}>${s}</option>`).join('')}</select>
      ${id === 'tier' ? `<label class="switch"><input type="checkbox" id="dir" ${state.config.display?.director ? 'checked' : ''}>主导者署名</label>` : ''}
      <div class="actions">
        <button class="btn primary" id="sv-png">保存 PNG</button>
        <button class="btn" id="cp-png">复制 PNG</button>
        <button class="btn" id="cp-svg">复制 SVG</button>
        ${id === 'tier' ? '<button class="btn" id="cp-embed">嵌入代码</button>' : ''}
        <button class="btn" id="cp-md">CREDITS.md</button>
      </div>
      <div class="meta" id="meta"></div>
    </aside>
  </div>`;
  $('#back').onclick = () => { state.view = 'gallery'; renderView(); };
  renderSwatches();
  v.querySelectorAll('[data-var]').forEach((b) => ((b as HTMLElement).onclick = () => {
    state.variant = (b as any).dataset.var;
    v.querySelectorAll('[data-var]').forEach((x) => x.classList.toggle('on', x === b));
    paintFocus();
  }));
  ($('#size') as HTMLSelectElement).onchange = (e) => { state.size = (e.target as HTMLSelectElement).value; paintFocus(); };
  ($('#dir') as HTMLInputElement | null)?.addEventListener('change', async (e) => {
    const on = (e.target as HTMLInputElement).checked;
    state.config.display = { ...(state.config.display ?? {}), director: on };
    state.model = buildModel(state.raw, { director: on, lang: 'zh' });
    await api('set_display_director', { projectId: state.current.id, on }).catch(() => {});
    paintFocus();
  });
  $('#cp-svg').onclick = () => copy(state.svg, 'SVG 已复制');
  $('#cp-png').onclick = async () => {
    try {
      const png = await toPng(state.svg);
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': new Blob([png as BlobPart], { type: 'image/png' }) })]);
      toast('PNG 已复制');
    } catch { toast('这个环境不能复制图片，用保存 PNG'); }
  };
  $('#sv-png').onclick = async () => {
    const png = await toPng(state.svg);
    const name = `${state.model!.project || 'credits'}-${id}${state.variant ? '-' + state.variant : ''}.png`;
    const path = await api('save_file', { name, bytes: b64(png), mime: 'image/png' });
    toast(path);
  };
  const embed = $('#cp-embed');
  if (embed) embed.onclick = async () => {
    const director = !!state.config.display?.director;
    const s = native
      ? await api('embed_snippet', { projectId: state.current.id, form: id, theme: state.theme, director })
      : `<script type="module" src="https://cdn.jsdelivr.net/npm/@creditia/embed@0/dist/creditia-card.js"><\/script>\n<creditia-card src=".creditia/credits.json" form="${id}" theme="${state.theme}"${director ? ' director' : ''}></creditia-card>`;
    copy(s, '嵌入代码已复制');
  };
  $('#cp-md').onclick = () => copy(creditsBlock(state.model!), 'CREDITS.md 区块已复制');
  paintFocus();
}
function renderSwatches() {
  const el = $('#themes');
  el.innerHTML = THEMES.map((t) => `<button data-theme="${t.id}" title="${esc(t.name)}" class="${t.id === state.theme ? 'on' : ''}"><i style="background:${t.bg}"></i><i style="background:${t.accent}"></i></button>`).join('');
  el.onclick = (e) => {
    const b = (e.target as HTMLElement).closest('button') as HTMLElement | null; if (!b) return;
    state.theme = b.dataset.theme!; localStorage.creditiaTheme = state.theme;
    el.querySelectorAll('button').forEach((x) => x.classList.toggle('on', x === b));
    const cur = state.variant ?? FORMS[state.focus!].def?.(theme());
    document.querySelectorAll('[data-var]').forEach((x) => x.classList.toggle('on', (x as HTMLElement).dataset.var === cur));
    paintFocus();
  };
}
async function paintFocus() {
  const out = $('#ex-out'); if (!out) return;
  const token = ++state.token;
  const id = state.focus!;
  const ctx = await ctxOf(id, state.variant);
  let svg: string | null;
  if (state.size === '原生') svg = await renderSvg(id, state.model!, theme(), ctx);
  else { const [w, h] = state.size.split('×').map(Number); svg = await renderSized(id, state.model!, theme(), w, h, ctx); }
  if (token !== state.token) return;
  state.svg = svg ?? '';
  out.innerHTML = svg ?? '<span class="ph">—</span>';
  $('#meta').textContent = svg ? `${state.theme} · ${state.size}` : '';
}

// ───────────────────────── 档案 ─────────────────────────
function viewArchive(v: HTMLElement) {
  const raw = state.raw, m = state.model!;
  const units: any[] = raw.units ?? [];
  const groups: { key: string; title: string; date?: string; units: any[] }[] = [];
  const unrel = units.filter((u) => !u.version);
  if (unrel.length) groups.push({ key: 'unreleased', title: '未发布', units: unrel });
  for (const ver of [...(raw.versions ?? [])].reverse()) { const us = units.filter((u) => u.version === ver.tag); if (us.length) groups.push({ key: ver.tag, title: ver.tag, date: ver.date, units: us }); }
  const uid = (id: string) => (id.startsWith('pr:') ? `#${id.slice(3)}` : id.slice(2));
  const shareBar = (u: any) => {
    const by = new Map<string, number>();
    for (const s of u.shares ?? []) { const c = (raw.contributors ?? []).find((x: any) => x.id === s.contributor); const k = c?.harness ?? (s.contributor === 'unattributed' ? 'unattributed' : 'human'); by.set(k, (by.get(k) ?? 0) + s.share); }
    return `<div class="ubar">${[...by.entries()].sort((a, b) => b[1] - a[1]).map(([k, s]) => `<i style="width:${(s * 100).toFixed(1)}%;background:${k === 'unattributed' ? 'repeating-linear-gradient(45deg,#CFCBC3 0 2px,#ECEAE5 2px 4px)' : famColor(k, theme())}" title="${esc(k)} ${Math.round(s * 100)}%"></i>`).join('')}</div>`;
  };
  v.innerHTML = `
  <div class="panel">
    <div class="phead"><h2>${esc(m.project)}</h2><span class="meta">${units.length} 项</span><div class="grow"></div>
      <button class="btn" id="cp-md">复制 CREDITS.md</button>
      <button class="btn" id="finder">在访达中显示</button></div>
    ${groups.length ? groups.map((g) => `<div class="ver"><h3>${esc(g.title)}${g.date ? ` · ${g.date}` : ''}<span>${g.units.length} 项</span></h3>
      ${[...g.units].sort((a, b) => String(b.date).localeCompare(String(a.date))).map((u) => `
      <div class="unit">
        <div class="uinfo"><span class="uid mono">${esc(uid(String(u.id)))}</span><span class="utitle">${esc(String(u.title ?? '').replace(/\s*\(#\d+\)\s*$/, ''))}</span><span class="ulead">${esc(unitLead(raw, u))}</span><span class="udate mono">${u.date}</span></div>
        ${shareBar(u)}
      </div>`).join('')}
    </div>`).join('') : '<div class="empty" style="margin-top:60px">还没有按项整理的记录</div>'}
  </div>`;
  $('#cp-md').onclick = () => copy(mergeCreditsMd(null, creditsBlock(m)), 'CREDITS.md 区块已复制');
  $('#finder').onclick = () => api('reveal', { path: state.current.path });
}

// ───────────────────────── 设置 ─────────────────────────
async function settingsSheet() {
  const el = $('#sheet');
  el.hidden = false;
  el.innerHTML = `<div class="sheet-back" id="sheet-back"></div>
  <div class="sheet">
    <h2>设置</h2>
    <h3>数据源</h3><div id="doc"><div class="loading">检查中…</div></div>
    <h3>作品库</h3><div class="row"><code id="libpath">…</code><button class="btn" id="lib-open">打开</button></div>
    <h3>分析</h3><div class="row"><code id="cmd">cd ${esc(state.current?.path ?? '…')} && creditia analyze</code><button class="btn" id="cp-cmd">复制</button></div>
    <div class="meta">creditia 0.1.0 · 证据库只存行哈希与元数据</div>
  </div>`;
  const close = () => { el.hidden = true; el.innerHTML = ''; document.removeEventListener('keydown', onKey); };
  const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') close(); };
  document.addEventListener('keydown', onKey);
  $('#sheet-back').onclick = close;
  $('#cp-cmd').onclick = () => copy(($('#cmd') as HTMLElement).textContent ?? '', '命令已复制');
  const lp = await api('library_path').catch(() => '');
  $('#libpath').textContent = lp || '浏览器调试模式没有作品库';
  $('#lib-open').onclick = () => (lp ? api('open_path', { path: lp }) : toast('浏览器调试模式没有作品库'));
  state.doctor = await api('doctor').catch(() => []);
  $('#doc').innerHTML = state.doctor.length
    ? state.doctor.map((d) => `<div class="src"><i class="dot ${d.ok ? 'ok' : ''}"></i><b>${esc(d.name)}</b><span class="path">${esc(d.path)}</span><span class="q">${esc(d.detail)}</span></div>`).join('')
    : '<div class="meta">没有可用的数据源</div>';
}

boot();
