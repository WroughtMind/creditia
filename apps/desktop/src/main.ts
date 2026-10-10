// creditia App：本机作品库浏览、统计、导出。渲染和 CLI 共用同一套 forms（satori → SVG，resvg-wasm → PNG）。
import { THEMES, themeById } from '../../../packages/render/src/themes.ts';
import { buildModel, fmt, pct, compact, type Model } from '../../../packages/render/src/data.ts';
import { FORMS, GROUPS, type Ctx } from '../../../packages/render/src/registry.ts';
import { creditsBlock, mergeCreditsMd, unitLead } from '../../../packages/render/src/text.ts';
import { famColor } from '../../../packages/render/src/brand.ts';
import { ready, renderSvg, renderSized, toPng } from './preview.ts';

// ───────────────────────── Tauri 桥 ─────────────────────────
const native = (window as any).__TAURI__?.core?.invoke as ((cmd: string, args?: any) => Promise<any>) | undefined;
async function api(cmd: string, args: any = {}): Promise<any> {
  if (native) return native(cmd, args);
  switch (cmd) {
    case 'projects': return (await fetch('/__dev/projects')).json();
    case 'credits': return (await fetch(`/__dev/credits?id=${encodeURIComponent(args.projectId)}`)).json();
    case 'config_of': return (await fetch(`/__dev/config`)).json();
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
type Tab = 'overview' | 'stats' | 'export' | 'archive' | 'settings';
const $ = <T extends HTMLElement = HTMLElement>(s: string) => document.querySelector(s) as T;
const state = {
  projects: [] as any[], current: null as any, config: {} as any,
  raw: null as any, model: null as Model | null,
  tab: 'overview' as Tab, theme: (localStorage.creditiaTheme as string) || 'minimal-light',
  form: 'tier' as string, variant: undefined as string | undefined, size: '原生',
  exportSvg: '', doctor: [] as any[], token: 0,
};
const theme = () => themeById(state.theme)!;
const esc = (s: unknown) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!);
function toast(msg: string) { const el = $('#toast'); el.textContent = msg; el.hidden = false; clearTimeout((toast as any)._t); (toast as any)._t = setTimeout(() => (el.hidden = true), 2800); }
async function copy(text: string, ok = '已复制') { try { await navigator.clipboard.writeText(text); toast(ok); } catch { toast('复制失败'); } }
const b64 = (bytes: Uint8Array) => { let s = ''; for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000)); return btoa(s); };
const b64bytes = (s: string) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));

// ───────────────────────── 启动与项目 ─────────────────────────
async function boot() {
  renderThemePicker();
  wireTabs();
  try { state.projects = await api('projects'); } catch (e: any) { $('#lib').textContent = String(e?.message ?? e); return; }
  renderProjects();
  $('#lib').textContent = state.projects.length ? `作品库 · ${state.projects.length} 个项目` : '作品库为空';
  if (state.projects.length) openProject(state.projects[0]);
  else showEmpty('作品库还是空的。先在仓库里运行 creditia analyze，再回到这里。');
  $('#q').addEventListener('input', renderProjects);
}
function renderProjects() {
  const q = ($('#q') as HTMLInputElement).value.trim().toLowerCase();
  const ul = $('#projects'); ul.innerHTML = '';
  for (const p of state.projects.filter((p) => !q || String(p.name).toLowerCase().includes(q))) {
    const li = document.createElement('li');
    li.className = p.id === state.current?.id ? 'on' : '';
    li.innerHTML = `<b>${esc(p.name)}</b><span>${p.lastAnalyzed ? new Date(p.lastAnalyzed * 1000).toLocaleDateString('zh-CN') : '未分析'} · ${fmt(p.evidence)} 条证据</span>`;
    li.onclick = () => openProject(p);
    ul.append(li);
  }
}
async function openProject(p: any) {
  state.current = p; state.raw = null; state.model = null; state.variant = undefined;
  $('#name').textContent = p.name;
  $('#sub').textContent = `${p.path} · 读取中…`;
  renderProjects();
  try {
    state.raw = await api('credits', { projectId: p.id });
    state.config = await api('config_of', { projectId: p.id }).catch(() => ({}));
  } catch (e: any) { showEmpty(String(e?.message ?? e)); return; }
  state.model = buildModel(state.raw, { director: !!state.config.display?.director, lang: 'zh' });
  $('#sub').textContent = `${p.path} · 生成于 ${String(state.raw.generatedAt ?? '').slice(0, 10)}`;
  renderView();
}
function showEmpty(msg: string) { $('#tabs').innerHTML = ''; $('#view').innerHTML = `<div class="empty">${esc(msg)}</div>`; }

// ───────────────────────── 顶部：风格 ─────────────────────────
function renderThemePicker() {
  const el = $('#themes');
  el.innerHTML = THEMES.map((t) => `<button data-theme="${t.id}" title="${esc(t.name)}" class="${t.id === state.theme ? 'on' : ''}"><i style="background:${t.bg}"></i><i style="background:${t.ink}"></i></button>`).join('');
  el.onclick = (e) => {
    const b = (e.target as HTMLElement).closest('button'); if (!b) return;
    state.theme = (b as any).dataset.theme; localStorage.creditiaTheme = state.theme;
    el.querySelectorAll('button').forEach((x) => x.classList.toggle('on', x === b));
    renderView();
  };
}

// ───────────────────────── 页签 ─────────────────────────
const TABS: [Tab, string][] = [['overview', '概览'], ['stats', '统计'], ['export', '导出'], ['archive', '档案'], ['settings', '设置']];
function wireTabs() {
  const el = $('#tabs');
  el.innerHTML = TABS.map(([id, n]) => `<button data-tab="${id}">${n}</button>`).join('');
  el.onclick = (e) => { const b = (e.target as HTMLElement).closest('button'); if (!b) return; state.tab = (b as any).dataset.tab; renderView(); };
}
function renderView() {
  $('#tabs').querySelectorAll('button').forEach((b) => b.classList.toggle('on', (b as any).dataset.tab === state.tab));
  const v = $('#view'); v.innerHTML = ''; v.scrollTop = 0;
  if (!state.model) return;
  ({ overview: viewOverview, stats: viewStats, export: viewExport, archive: viewArchive, settings: viewSettings }[state.tab])(v);
}

// ───────────────────────── 概览 ─────────────────────────
function viewOverview(v: HTMLElement) {
  const m = state.model!;
  const kpi: [string, string][] = [['提交', fmt(m.commits)], ['项', fmt(m.units)], ['新增行', fmt(m.linesAdded)], ['留下', fmt(m.linesSurviving)], ['会话', m.sessions ? fmt(m.sessions) : '—'], ['Token', m.tokens ? compact(m.tokens) : '—'], ['工期', m.spanDays ? `${m.spanDays} 天` : '—']];
  v.innerHTML = `
    <div class="kpis">${kpi.map(([k, x]) => `<div class="kpi"><b>${x}</b><span>${k}</span></div>`).join('')}</div>
    <div class="panel">
      <div class="phead"><h2>${esc(m.project)}</h2><div class="grow"></div>
        <label class="switch"><input type="checkbox" id="dir" ${state.config.display?.director ? 'checked' : ''}> 主导者署名</label>
        <button class="btn" id="copy-embed">复制嵌入代码</button></div>
      <div id="tier-out" class="tier-out"><div class="loading">渲染中…</div></div>
    </div>
    <div class="panel"><h2>证据覆盖</h2>${coverageHtml()}</div>`;
  ($('#dir') as HTMLInputElement).addEventListener('change', async (e) => {
    const on = (e.target as HTMLInputElement).checked;
    state.config.display = { ...(state.config.display ?? {}), director: on };
    state.model = buildModel(state.raw, { director: on, lang: 'zh' });
    await api('set_display_director', { projectId: state.current.id, on }).catch(() => {});
    paintTier();
  });
  $('#copy-embed').onclick = async () => {
    const director = !!state.config.display?.director;
    const s = native
      ? await api('embed_snippet', { projectId: state.current.id, form: 'tier', theme: state.theme, director })
      : `<script type="module" src="https://cdn.jsdelivr.net/npm/@creditia/embed@0/dist/creditia-card.js"><\/script>\n<creditia-card src=".creditia/credits.json" form="tier" theme="${state.theme}"${director ? ' director' : ''}></creditia-card>`;
    copy(s, '嵌入代码已复制');
  };
  paintTier();
}
async function paintTier() {
  const el = $('#tier-out'); if (!el) return;
  const token = ++state.token;
  const svg = await renderSvg('tier', state.model!, theme(), { lang: 'zh' });
  if (token !== state.token) return;
  el.innerHTML = svg ?? '<p class="note">这个项目还没有可画的 tier。</p>';
}
function coverageHtml() {
  const cov: any[] = state.raw?.coverage ?? [];
  if (!cov.length) return '<p class="note">全部区间都有逐行日志。</p>';
  const label = (b: string) => ({ verified: '逐行对上', manual: '手动标注', declared: '提交信息吻合', inferred: '按分支推断' }[b] ?? b);
  const color: Record<string, string> = { verified: '#161514', manual: '#3E7C59', declared: '#8A6FB8', inferred: '#AEAAA3' };
  const days = (a: string, b: string) => Math.max(1, Math.round((Date.parse(b) - Date.parse(a)) / 86400000) + 1);
  return `<div class="cov">${cov.map((c) => `<div class="cov-seg" style="flex:${days(c.from, c.to)};background:${color[c.best] ?? '#AEAAA3'}" title="${c.from} — ${c.to}：${label(c.best)}"><span>${label(c.best)}</span></div>`).join('')}</div>
    <div class="cov-axis"><span>${cov[0].from}</span><span>${cov[cov.length - 1].to}</span></div>
    <p class="note">颜色越深，证据越直接；悬停看每段的日期。「未确认」不在这里，它始终单独计算。</p>`;
}

// ───────────────────────── 统计 ─────────────────────────
const STAT_FORMS = ['waffle', 'donut', 'area', 'ribbon', 'lanes', 'clock', 'versions', 'settle', 'mosaic'];
function viewStats(v: HTMLElement) {
  v.innerHTML = `<div class="stats">${STAT_FORMS.map((id) => `<div class="panel stat"><div class="phead"><h2>${FORMS[id].name}</h2><span class="q">${FORMS[id].q}</span></div><div class="stat-out" data-form="${id}"><div class="loading">渲染中…</div></div></div>`).join('')}</div>`;
  const token = ++state.token;
  for (const id of STAT_FORMS) {
    renderSvg(id, state.model!, theme(), { lang: 'zh' }).then((svg) => {
      if (token !== state.token) return;
      const el = v.querySelector(`[data-form="${id}"]`);
      if (el) el.innerHTML = svg ?? '<p class="note">这个项目没有这项数据。</p>';
    });
  }
}

// ───────────────────────── 导出 ─────────────────────────
function viewExport(v: HTMLElement) {
  const f = FORMS[state.form] ?? FORMS.tier;
  const variants = f.variants ?? [];
  v.innerHTML = `
  <div class="export">
    <aside class="ex-left panel">
      ${GROUPS.map(([g, ids]) => `<h3>${g}</h3><div class="chips">${ids.map((id) => `<button class="chip ${id === state.form ? 'on' : ''}" data-form="${id}">${FORMS[id].name}</button>`).join('')}</div>`).join('')}
      <h3>版式</h3><div class="chips">${variants.length ? variants.map((x) => `<button class="chip ${x === state.variant ? 'on' : ''}" data-var="${x}">${x}</button>`).join('') : '<span class="q">这个形式只有一种版式</span>'}</div>
      <h3>尺寸</h3>
      <select id="size">${['原生', '1200×630', '1080×1350', '1080×1920', '1080×1080'].map((s) => `<option ${s === state.size ? 'selected' : ''}>${s}</option>`).join('')}</select>
      <h3>风格</h3><p class="note">用右上角的七块色板切换，预览会跟着变。</p>
    </aside>
    <div class="ex-right panel">
      <div id="ex-out" class="ex-out"><div class="loading">渲染中…</div></div>
      <div class="ex-actions">
        <button class="btn" id="cp-svg">复制 SVG</button>
        <button class="btn" id="cp-png">复制 PNG</button>
        <button class="btn primary" id="sv-png">保存 PNG</button>
        <button class="btn" id="cp-embed" ${state.form === 'tier' ? '' : 'disabled'}>复制嵌入代码</button>
        <button class="btn" id="cp-md">复制 CREDITS.md</button>
      </div>
      <p class="note" id="ex-meta"></p>
    </div>
  </div>`;
  v.querySelectorAll('[data-form]').forEach((b) => ((b as HTMLElement).onclick = () => { state.form = (b as any).dataset.form; state.variant = undefined; renderView(); }));
  v.querySelectorAll('[data-var]').forEach((b) => ((b as HTMLElement).onclick = () => { state.variant = (b as any).dataset.var; renderView(); }));
  ($('#size') as HTMLSelectElement).onchange = (e) => { state.size = (e.target as HTMLSelectElement).value; paintExport(); };
  $('#cp-svg').onclick = () => copy(state.exportSvg, 'SVG 已复制');
  $('#cp-png').onclick = async () => {
    try {
      const png = await toPng(state.exportSvg);
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': new Blob([png as BlobPart], { type: 'image/png' }) })]);
      toast('PNG 已复制');
    } catch { toast('这个环境不能直接复制图片，改用「保存 PNG」'); }
  };
  $('#sv-png').onclick = async () => {
    const png = await toPng(state.exportSvg);
    const name = `${state.model!.project || 'credits'}-${state.form}${state.variant ? '-' + state.variant : ''}.png`;
    const path = await api('save_file', { name, bytes: b64(png), mime: 'image/png' });
    toast(`已保存：${path}`);
  };
  $('#cp-embed').onclick = async () => {
    const director = !!state.config.display?.director;
    const s = native
      ? await api('embed_snippet', { projectId: state.current.id, form: state.form, theme: state.theme, director })
      : `<script type="module" src="https://cdn.jsdelivr.net/npm/@creditia/embed@0/dist/creditia-card.js"><\/script>\n<creditia-card src=".creditia/credits.json" form="tier" theme="${state.theme}"></creditia-card>`;
    copy(s, '嵌入代码已复制');
  };
  $('#cp-md').onclick = () => copy(creditsBlock(state.model!), 'CREDITS.md 区块已复制');
  paintExport();
}
async function paintExport() {
  const out = $('#ex-out'); if (!out) return;
  const token = ++state.token;
  const ctx: Ctx = { lang: 'zh', variant: state.variant };
  let svg: string | null;
  if (state.size === '原生') svg = await renderSvg(state.form, state.model!, theme(), ctx);
  else { const [w, h] = state.size.split('×').map(Number); svg = await renderSized(state.form, state.model!, theme(), w, h, ctx); }
  if (token !== state.token) return;
  state.exportSvg = svg ?? '';
  out.innerHTML = svg ?? '<p class="note">这个形式需要的数据不够（例如没有 tag 或会话记录），换一个试试。</p>';
  $('#ex-meta').textContent = svg ? `${state.form}${state.variant ? ` · ${state.variant}` : ''} · ${theme().id} · ${state.size}` : '';
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
    <div class="phead"><h2>档案</h2><div class="grow"></div>
      <button class="btn" id="cp-md">复制 CREDITS.md</button>
      ${state.current?.hasCredits ? '<button class="btn" id="open-md">打开 CREDITS.md</button>' : ''}
      <button class="btn" id="finder">在访达中显示</button></div>
    ${groups.length ? groups.map((g) => `<div class="ver"><h3>${esc(g.title)}${g.date ? ` · ${g.date}` : ''}<span class="q">${g.units.length} 项</span></h3>
      ${[...g.units].sort((a, b) => String(b.date).localeCompare(String(a.date))).map((u) => `
      <div class="unit">
        <div class="uinfo"><span class="uid">${esc(uid(String(u.id)))}</span><span class="utitle">${esc(String(u.title ?? '').replace(/\s*\(#\d+\)\s*$/, ''))}</span><span class="ulead">${esc(unitLead(raw, u))}</span><span class="udate">${u.date}</span></div>
        ${shareBar(u)}
      </div>`).join('')}
    </div>`).join('') : '<p class="note">还没有按项整理的记录。</p>'}
  </div>`;
  $('#cp-md').onclick = () => copy(mergeCreditsMd(null, creditsBlock(m)), 'CREDITS.md 区块已复制');
  $('#open-md')?.addEventListener('click', () => api('open_path', { path: `${state.current.path}/CREDITS.md` }));
  $('#finder').onclick = () => api('reveal', { path: state.current.path });
}

// ───────────────────────── 设置 ─────────────────────────
async function viewSettings(v: HTMLElement) {
  v.innerHTML = `
  <div class="panel"><h2>数据源</h2><div id="doc"><div class="loading">检查中…</div></div><p class="note">全部只读。doctor 的输出和 CLI 的 creditia doctor 一致。</p></div>
  <div class="panel"><h2>作品库</h2><p class="note" id="libpath">…</p><p class="cmd"><button class="btn" id="lib-open">打开作品库文件夹</button></p></div>
  <div class="panel"><h2>分析</h2><p class="note">分析由 CLI 完成，App 只读结果；发出的是同一套 Rust 核心。</p><div class="cmd"><code id="cmd">cd ${esc(state.current?.path ?? '…')} && creditia analyze</code><button class="btn" id="cp-cmd">复制</button></div></div>
  <div class="panel"><h2>关于</h2><p class="note">creditia 0.1.0 · 为你和 AI 一起做出的作品署名。证据库只存行哈希和元数据，不存代码和 prompt。</p></div>`;
  $('#cp-cmd').onclick = () => copy(($('#cmd') as HTMLElement).textContent ?? '', '命令已复制');
  state.doctor = await api('doctor').catch(() => []);
  $('#doc').innerHTML = state.doctor.length
    ? state.doctor.map((d) => `<div class="src"><i class="dot ${d.ok ? 'ok' : ''}"></i><b>${esc(d.name)}</b><span class="path">${esc(d.path)}</span><span class="q">${esc(d.detail)}</span></div>`).join('')
    : '<p class="note">没有可用的数据源。</p>';
  const lp = await api('library_path').catch(() => '');
  $('#libpath').textContent = lp || '浏览器调试模式下没有作品库';
  $('#lib-open').onclick = () => (lp ? api('open_path', { path: lp }) : toast('浏览器调试模式没有作品库'));
}

boot();
