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
  $('#add').onclick = addSheet;
  $('#nav').onclick = (e) => {
    const b = (e.target as HTMLElement).closest('button[data-view]') as HTMLElement | null; if (!b) return;
    state.view = b.dataset.view as View; renderView();
  };
  $('#q').addEventListener('input', renderProjects);
  try { state.projects = await api('projects'); } catch (e: any) { return firstRun(`没能读取项目列表：${e?.message ?? e}`); }
  renderProjects();
  if (state.projects.length) openProject(state.projects[0]); else firstRun();
}

/** 第一次打开、还没有任何项目时：说清楚这是什么、下一步做什么 */
function firstRun(err?: string) {
  $('#name').textContent = '欢迎使用 creditia';
  $('#sum').textContent = '它会看你项目的 git 提交和本机 AI 编程工具的记录，算出每段代码是谁写的，再做成能分享的图。';
  $('#nav').hidden = true;
  $('#view').innerHTML = `
  <div class="welcome">
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

function renderProjects() {
  const q = ($('#q') as HTMLInputElement).value.trim().toLowerCase();
  ($('#q') as HTMLInputElement).hidden = state.projects.length < 6;
  const ul = $('#projects'); ul.innerHTML = '';
  for (const p of state.projects.filter((p) => !q || String(p.name).toLowerCase().includes(q))) {
    const li = document.createElement('li');
    li.className = p.id === state.current?.id ? 'on' : '';
    const when = p.lastAnalyzed ? new Date(p.lastAnalyzed * 1000).toLocaleDateString('zh-CN', { month: 'long', day: 'numeric' }) : '';
    li.innerHTML = `<b>${esc(p.name)}</b><span>${when ? `${when}分析` : '还没分析'}</span>`;
    li.title = p.path;
    li.onclick = () => openProject(p);
    ul.append(li);
  }
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
  $('#name').textContent = p.name; $('#sum').textContent = '正在读取…';
  renderProjects();
  try {
    state.raw = await api('credits', { projectId: p.id });
    state.config = await api('config_of', { projectId: p.id }).catch(() => ({}));
  } catch (e: any) {
    $('#sum').textContent = '';
    $('#view').innerHTML = `<div class="empty"><p>这个项目还没有分析结果。</p><div class="row"><code>${esc(analyzeCmd(p.path))}</code><button class="btn" id="cp-an">复制</button></div><p class="meta">${esc(e?.message ?? e)}</p></div>`;
    $('#cp-an').onclick = () => copy(analyzeCmd(p.path), '命令已复制');
    return;
  }
  state.model = buildModel(state.raw, { director: !!state.config.display?.director, lang: 'zh' });
  $('#name').textContent = state.model.project || p.name;
  $('#sum').textContent = summary(state.model);
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

function viewMake(v: HTMLElement) {
  v.innerHTML = `
  <div class="make">
    <div class="canvas">
      <div class="stage" id="stage"><div class="loading">正在画…</div></div>
      <div class="caption" id="caption"></div>
    </div>
    <aside class="rail">
      <section>
        <h3><i>1</i>选一张图</h3>
        <div class="thumbs" id="thumbs">${GROUPS.map(([g, ids]) => {
          const list = ids.filter((id) => ORDER.includes(id));
          return list.length ? `<div class="tgroup"><h4>${esc(g)}</h4><div class="tgrid">${list.map((id) => `
            <button class="thumb ${id === state.form ? 'on' : ''}" data-form="${id}" title="${esc(FORMS[id].q)}">
              <span class="tstage" style="aspect-ratio:${aspectOf(id)}" data-tstage="${id}"></span>
              <span class="tname">${esc(FORMS[id].name)}</span>
            </button>`).join('')}</div></div>` : '';
        }).join('')}</div>
      </section>
      <section>
        <h3><i>2</i>选风格</h3>
        <div class="themes" id="themes">${THEMES.map((t) => `
          <button data-theme="${t.id}" class="${t.id === state.theme ? 'on' : ''}">
            <span class="sw" style="background:${t.bg}"><i style="background:${t.accent}"></i></span>
            <span>${esc(THEME_ZH[t.id] ?? t.name)}</span>
          </button>`).join('')}</div>
        <div id="variants"></div>
        <label class="field"><span>尺寸</span><select id="size">${SIZES.map(([k, n]) => `<option value="${k}" ${k === state.size ? 'selected' : ''}>${n}</option>`).join('')}</select></label>
        <div id="dir-slot"></div>
      </section>
      <section class="save">
        <h3><i>3</i>拿走</h3>
        <button class="btn primary big" id="sv-png">保存图片</button>
        <div class="more">
          <button class="btn" id="cp-png">复制图片</button>
          <button class="btn" id="cp-svg">复制 SVG</button>
          <button class="btn" id="cp-embed" title="贴到网页里，会自动读取 credits.json 显示这张图">网页嵌入代码</button>
        </div>
      </section>
    </aside>
  </div>`;
  v.querySelector('#thumbs')!.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest('[data-form]') as HTMLElement | null; if (!b) return;
    state.form = b.dataset.form!; localStorage.creditiaForm = state.form; state.variant = undefined;
    v.querySelectorAll('[data-form]').forEach((x) => x.classList.toggle('on', x === b));
    renderOptions(); paintFocus();
  });
  $('#themes').onclick = (e) => {
    const b = (e.target as HTMLElement).closest('button') as HTMLElement | null; if (!b) return;
    state.theme = b.dataset.theme!; localStorage.creditiaTheme = state.theme;
    $('#themes').querySelectorAll('button').forEach((x) => x.classList.toggle('on', x === b));
    renderOptions(); paintFocus(); paintThumbs();
  };
  ($('#size') as HTMLSelectElement).onchange = (e) => { state.size = (e.target as HTMLSelectElement).value; paintFocus(); };
  $('#sv-png').onclick = savePng;
  $('#cp-png').onclick = copyPng;
  $('#cp-svg').onclick = () => copy(state.svg, 'SVG 已复制');
  $('#cp-embed').onclick = copyEmbed;
  renderOptions(); paintFocus(); paintThumbs();
}

/** 版式与主导者开关：跟着当前图走 */
function renderOptions() {
  const f = FORMS[state.form];
  const vs = f.variants ?? []; const cur = curVariant();
  $('#variants').innerHTML = vs.length ? `<div class="field"><span>版式</span><div class="chips">${vs.map((x) => `<button class="chip ${x === cur ? 'on' : ''}" data-var="${x}">${esc(VARIANT_ZH[state.form]?.[x] ?? x)}</button>`).join('')}</div></div>` : '';
  $('#variants').onclick = (e) => {
    const b = (e.target as HTMLElement).closest('[data-var]') as HTMLElement | null; if (!b) return;
    state.variant = b.dataset.var; $('#variants').querySelectorAll('[data-var]').forEach((x) => x.classList.toggle('on', x === b)); paintFocus();
  };
  const tierLike = state.form === 'tier';
  $('#dir-slot').innerHTML = tierLike ? `<label class="switch"><input type="checkbox" id="dir" ${state.config.display?.director ? 'checked' : ''}><span>在图上写明我是主导者</span></label>` : '';
  ($('#dir') as HTMLInputElement | null)?.addEventListener('change', async (e) => {
    const on = (e.target as HTMLInputElement).checked;
    state.config.display = { ...(state.config.display ?? {}), director: on };
    state.model = buildModel(state.raw, { director: on, lang: 'zh' });
    await api('set_display_director', { projectId: state.current.id, on }).catch(() => {});
    paintFocus();
  });
  ($('#cp-embed') as HTMLButtonElement).hidden = state.form !== 'tier';
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
  const id = state.form; const f = FORMS[id];
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
  const v = curVariant();
  $('#caption').innerHTML = `<b>${esc(f.name)}</b><span>${esc(f.q)}</span><span class="dim">${esc(THEME_ZH[state.theme] ?? state.theme)}${v ? ` · ${esc(VARIANT_ZH[id]?.[v] ?? v)}` : ''}</span>`;
  for (const id2 of ['sv-png', 'cp-png', 'cp-svg']) ($(`#${id2}`) as HTMLButtonElement).disabled = !svg;
}
function whyEmpty(id: string) {
  if (id === 'profile') return '需要至少一个分析过的项目。';
  if (id === 'lanes') return '需要本机有 agent 会话记录。';
  if (id === 'versions') return '需要项目打过版本 tag。';
  return '数据太少，换一张图试试。';
}

/** 缩略图：滚到眼前才画，大小按每张图的真实比例 */
let io: IntersectionObserver | null = null;
function paintThumbs() {
  const token = ++state.thumbToken;
  io?.disconnect();
  const queue: string[] = []; let busy = 0;
  const pump = async () => {
    while (busy < 2 && queue.length) {
      const id = queue.shift()!; busy++;
      (async () => {
        let svg: string | null = null;
        try { svg = await renderSvg(id, state.model!, theme(), await ctxOf(id)); } catch { svg = null; }
        busy--;
        if (token !== state.thumbToken) return;
        const el = document.querySelector(`[data-tstage="${id}"]`) as HTMLElement | null;
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
  }, { root: document.querySelector('.rail'), rootMargin: '200px' });
  document.querySelectorAll('[data-tstage]').forEach((el) => { el.innerHTML = ''; io!.observe(el); });
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
  <div class="panel detail">
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
