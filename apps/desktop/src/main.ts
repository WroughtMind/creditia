// creditia App：一面展墙。顶部切换项目，左下角圆点切换风格，所有作品直接摆出来；
// 悬停看细节、换版式、选格式导出；拖起作品时下面浮出导出坞，松手就导出。
import { buildModel, type Model } from '../../../packages/render/src/classic/data.ts';
import { nameOf, resolveGroup } from '../../../packages/render/src/data.ts';
import { MODEL_VARIANTS } from '../../../packages/render/src/registry.ts';
import { mergeCredits } from '../../../packages/render/src/merge.ts';
import { creditsBlock, mergeCreditsMd } from '../../../packages/render/src/classic/text.ts';
import { FORMATS, SIZES, type Art } from '../../../packages/render/src/export.ts';
import { INTERACT_CSS, mountHits } from '../../../packages/render/src/interact.ts';
import { KITS, CLASSIC_COUNT, kitById, bootKits, setGroup, groupOf, hasModels, type Kit } from './kits.ts';
import { exportArt, type Format } from './engine.ts';
import { mountRoamPreview, releaseRoamPreview } from './roam-preview.ts';
import { RoamViewport } from './roam-viewport.ts';
import { wallTiles, tileKey as keyOf, wallVariant, selectWallVariant } from './wall.ts';
import { LOGOS, DEFAULT_LOGO, logoSvg } from './logo.ts';
import { markPaths, sharePetals, type Petal } from '../../../packages/render/src/mark.ts';

// ───────────────────────── 桥：Tauri 或浏览器调试 ─────────────────────────
const native = (window as any).__TAURI__?.core?.invoke as ((cmd: string, args?: any) => Promise<any>) | undefined;
/** 官网上的演示：页面先放好样例档案（window.__CREDITIA_DEMO__），导出的文件交给外层页面摆到「桌面」上 */
const demo = (window as any).__CREDITIA_DEMO__ as { projects: any[]; credits: Record<string, any>; doctor: any[] } | undefined;
const devCollect = { on: true, running: false, every: 3600, last: { at: Math.floor(Date.now() / 1000) - 1260, projects: 3, failed: 0, newEvidence: 128 } as any, agent: { supported: true, on: false } };
const devPlugs: Plugs = {
  hook: { installed: false, decided: null, path: '.git/hooks/prepare-commit-msg' },
  release: { installed: false, decided: null, path: '.github/workflows/creditia.yml', github: true, published: true },
};
/** 开发时加 ?fresh：作品库是空的，从第一次打开走起 */
const devFresh = { on: !demo && new URLSearchParams(location.search).has('fresh'), added: false };
async function api(cmd: string, args: any = {}): Promise<any> {
  if (native) return native(cmd, args);
  const get = (u: string) => fetch(u).then((r) => { if (!r.ok) throw new Error(r.statusText); return r.json(); });
  switch (cmd) {
    case 'projects': return demo ? demo.projects : devFresh.on && !devFresh.added ? [] : get('/__dev/projects');
    case 'discover': if (demo) throw new Error('demo'); await new Promise((r) => setTimeout(r, 900)); return { projects: [
      { name: 'catnap', path: '/Users/me/code/catnap', tools: { 'claude-code': 42, codex: 12 }, lastActive: '2026-10-20', exists: true, inLibrary: !devFresh.on },
      { name: 'atlas', path: '/Users/me/code/atlas', tools: { cursor: 30 }, lastActive: '2026-10-12', exists: true, inLibrary: false },
      { name: 'dotfiles', path: '/Users/me/dotfiles', tools: { 'claude-code': 4 }, lastActive: '2026-09-02', exists: true, inLibrary: false },
      { name: 'scratch', path: '/Users/me/tmp/scratch', tools: { codex: 3 }, lastActive: '2026-08-30', exists: true, inLibrary: false },
    ] };
    case 'add_project': if (demo) throw new Error('demo'); await new Promise((r) => setTimeout(r, 2500)); devFresh.added = true; return { id: (await get('/__dev/projects'))[0]?.id, result: {} };
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
    // 浏览器里（开发、官网演示）没有仓库可写：开关只在内存里拨
    case 'integrations': return structuredClone(devPlugs);
    // 后台采集：开发时给一份样子；官网演示里没有，设置里不出这一行
    case 'collect_info': if (demo) throw new Error('demo'); return structuredClone(devCollect);
    case 'set_collect': devCollect.on = args.on; return structuredClone(devCollect);
    case 'set_collect_agent': devCollect.agent.on = args.on; return structuredClone(devCollect);
    case 'logo': return store.get('logo') ?? DEFAULT_LOGO;
    case 'set_logo': return { logo: args.id };
    case 'collect_now': await new Promise((r) => setTimeout(r, 1200)); devCollect.last = { at: Math.floor(Date.now() / 1000), projects: 3, failed: 0, newEvidence: 0 }; return devCollect.last;
    case 'set_integration': {
      const k = args.which as 'hook' | 'release';
      devPlugs[k] = { ...devPlugs[k], installed: args.on, decided: args.on };
      return { installed: args.on, summary: args.on ? L(`已写入 ${devPlugs[k].path}`, `Wrote ${devPlugs[k].path}`) : L('已关闭', 'Turned off') };
    }
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
// 星号 *：标志本身，也是每个项目自己的 *（瓣长是各工具的份额）。等待时五瓣轮流亮，出结果时一瓣瓣长出来
const markEl = (petals?: Petal[] | null, cls = '') => `<svg class="mk${cls ? ' ' + cls : ''}" viewBox="0 0 24 24" aria-hidden="true">${markPaths({ petals: petals ?? undefined })}</svg>`;
const SPIN = () => markEl(null, 'spin');
/** 还没分析过的项目：五瓣淡淡的短瓣 */
const GHOST: Petal[] = Array.from({ length: 5 }, () => ({ color: 'currentColor', len: 0.5, opacity: 0.35 }));
const petalsOf = (m: Model) => sharePetals(m.agents.filter((a) => a.kind === 'agent').map((a) => ({ tool: a.key.split('/')[0], lines: a.lines })));
/** 每个项目的 * 记在本机，菜单里不用每次都读档案 */
const markOf = (id: string): Petal[] | null => { try { return JSON.parse(store.get('mark.' + id) ?? 'null'); } catch { return null; } };
type Act = { label: string; run: () => void };
let toastTimer = 0, toastMs = 0;
/** 提示条：可以带按钮（在访达中显示、取消……）、转圈的 *、或者一个项目的 *；ms 为 0 就一直留着。鼠标停在上面时不消失 */
function toast(msg: string, ms = 2600, o: { acts?: Act[]; spin?: boolean; mark?: Petal[] | null } = {}) {
  const el = $('#toast');
  el.innerHTML = `${o.spin ? SPIN() : o.mark ? markEl(o.mark, 'bloom') : ''}<span class="msg"></span>${(o.acts ?? []).map((a, i) => `<button data-i="${i}">${esc(a.label)}</button>`).join('')}`;
  el.querySelector('.msg')!.textContent = msg;
  el.onclick = (e) => { const b = (e.target as HTMLElement).closest('button'); if (!b) return; el.hidden = true; o.acts![Number(b.dataset.i)].run(); };
  el.hidden = false;
  toastMs = ms; clearTimeout(toastTimer);
  if (ms > 0) toastTimer = setTimeout(() => (el.hidden = true), ms);
}
/** 原生端的错误只带代码和细节，文案在这里按语言写 */
const ERRORS: Record<string, [string, string]> = {
  not_in_library: ['未找到该项目记录', 'This project is not in the library'],
  no_credits: ['这个项目还没分析过', 'Not analyzed yet'],
  dir_missing: ['项目目录已经不在了', 'The project folder is gone'],
  core_missing: ['找不到分析核心 creditia-core', 'Cannot find the analysis core (creditia-core)'],
  core_failed: ['分析失败', 'Analysis failed'],
  busy: ['已经有一个分析在跑', 'An analysis is already running'],
  cancelled: ['分析已取消', 'Analysis cancelled'],
  no_home: ['找不到用户目录', 'Cannot find your home folder'],
  io: ['读写文件失败', 'Could not read or write the file'],
  store: ['读取项目数据库失败', 'Cannot read the library'],
  not_allowed: ['只能打开这次导出的文件', 'Only files exported in this session can be shown'],
  switch_failed: ['没能改这一项', "Couldn't change this"],
  agent_failed: ['没能装上后台任务', "Couldn't set up the background task"],
  unsupported: ['这个系统上还不支持', 'Not supported on this system yet'],
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

const tiles = () => wallTiles(state.style);

// ───────────────────────── 启动 ─────────────────────────
async function boot() {
  bootKits(); state.style.warm();
  const st = document.createElement('style'); st.textContent = INTERACT_CSS; document.head.appendChild(st);
  if (native && OS === 'mac') document.body.classList.add('mac');
  if (native) void api('logo').then((id) => store.set('logo', String(id))).catch(() => null);
  applyLang(); renderDots(); renderDock(); wireTop(); wireRoam();
  // 滚动之后顶栏垫一层渐隐的玻璃，作品不再直接透到项目名和按钮后面
  const stageEl = $('#stage');
  stageEl.addEventListener('scroll', () => document.body.classList.toggle('scrolled', stageEl.scrollTop > 8), { passive: true });
  try { state.projects = await api('projects'); } catch (e: any) { return empty(() => [L('无法读取本地项目记录', 'Cannot read local projects'), errText(e)]); }
  if (!state.projects.length) return welcome();
  const last = (store.get('project') === ALL_ID && state.projects.length > 1 ? allProject() : null) ?? state.projects.find((p) => p.id === store.get('project')) ?? state.projects[0];
  await openProject(last);
  let resizeFrame = 0, columnWidth = rcw(), viewportWidth = innerWidth, viewportHeight = innerHeight;
  addEventListener('resize', () => {
    cancelAnimationFrame(resizeFrame);
    resizeFrame = requestAnimationFrame(() => {
      layout();
      const width = rcw();
      if (roam.ready) resizeRoam(columnWidth, viewportWidth, viewportHeight);
      columnWidth = width; viewportWidth = innerWidth; viewportHeight = innerHeight;
    });
  });
}
/** 空状态。传进来的是函数：换语言时按新语言重写一遍 */
type EmptyView = () => [title: string, detail?: string, cmd?: string, act?: Act];
let emptyView: EmptyView | null = null;
function empty(view: EmptyView, keepName = false) {
  emptyView = view;
  const [title, detail = '', cmd = '', act] = view();
  $('#board').innerHTML = ''; document.body.classList.remove('busy');
  const el = $('#empty'); el.hidden = false;
  applyView();
  el.innerHTML = `<div><b>${esc(title)}</b>${esc(detail)}${act ? `<br><button class="go">${esc(act.label)}</button>` : cmd ? `<br><code>${esc(cmd)}</code>` : ''}</div>`;
  if (act) (el.querySelector('.go') as HTMLElement).onclick = act.run;
  if (!keepName) $('#pname').textContent = 'creditia';
}

async function openProject(p: any) {
  const seq = ++state.openSeq;
  state.current = p; store.set('project', p.id);
  state.raw = null; state.model = null; state.arts.clear(); state.variants = {};
  roamReset(); // 读新档案前释放旧漫游，旧项目排队的预览不再占用绘图队列
  $('#group').hidden = true; plugs = null; renderPlugs();
  ++state.token; // 让正在画的旧项目停下
  $('#board').innerHTML = ''; slotWall(); // 档案还在读，先把墙的样子摆出来
  $('#pname').textContent = p.name;
  let raw: any;
  const st = readStamp(p); // 先记下版本再读档案：读的过程中又变了，下次比对还能发现
  try { raw = await creditsOf(p); }
  catch (e: any) {
    if (seq !== state.openSeq) return;
    if (e?.code && e.code !== 'no_credits') return empty(() => [L('读不到这个项目', 'Cannot read this project'), errText(e)], true);
    const can = native && p.exists !== false;
    return empty(() => [L('这个项目还没分析过', 'Not analyzed yet'), '', `cd ${p.path} && creditia analyze`, can ? { label: L('现在分析', 'Analyze now'), run: () => reanalyze(p) } : undefined], true);
  }
  if (seq !== state.openSeq) return; // 已经切到别的项目
  stampOf = await st;
  state.raw = raw;
  $('#empty').hidden = true; emptyView = null;
  setGroup(groupPref());
  rebuildModel();
  if (!p.all) loadPlugs(p);
  await paint();
  void refreshIfChanged(); // 打开时就看一眼有没有新提交
}

// ───────────────────────── 档案变了就原地刷新 ─────────────────────────
// 在终端里 creditia analyze、提交时的 hook 都会改档案。App 回到前台、或者每 30 秒，比一下版本；
// 变了就读新档案，墙上的作品原地换掉（不清空、不跳回顶部），换过的版式也留着。
// 档案没变、但仓库里有了新提交，就在后台跟上（keepUp）。
let stampOf = '', refreshing = false;
const readStamp = (p: any): Promise<string> => (!native ? Promise.resolve('')
  : p.all ? Promise.all(state.projects.map((x) => api('stamp', { projectId: x.id }).then(String, () => ''))).then((a) => a.join('|'))
  : api('stamp', { projectId: p.id }).then(String, () => ''));

// ───────────────────────── 全部项目 ─────────────────────────
// 作品库里不止一个项目时，项目菜单最上面有「全部项目」：各项目的档案并成一份（行数相加，文件和 PR 带上项目名），
// 整面墙照常画，按工具 / 按模型也照常切。读不到档案的项目跳过
const ALL_ID = '__all__';
const allProject = () => ({ id: ALL_ID, all: true, name: L('全部项目', 'All projects'), path: '', exists: true });
async function creditsOf(p: any): Promise<any> {
  if (!p.all) return api('credits', { projectId: p.id });
  const got = await Promise.all(state.projects.map((x) => api('credits', { projectId: x.id }).then((raw: any) => ({ name: x.name, raw }), () => null)));
  const parts = got.filter((x): x is { name: string; raw: any } => !!x);
  if (!parts.length) throw { code: 'no_credits' };
  return mergeCredits(parts, state.lang);
}
async function refreshIfChanged() {
  const p = state.current;
  if (!native || !p || !state.raw || refreshing || analyzing || document.hidden) return;
  refreshing = true;
  try {
    const st = await readStamp(p);
    if (!st || st === stampOf || p !== state.current) { await keepUp(p); return; }
    const raw = await creditsOf(p);
    if (p !== state.current) return;
    stampOf = st;
    await swapRaw(raw);
    state.projects = await api('projects').catch(() => state.projects);
    toast(L('已更新到最新提交', 'Updated to the latest commits'));
  } catch { /* 读不到就等下一次 */ } finally { refreshing = false; }
}
/** 跟上新提交：仓库里有了档案之后的新提交，就在后台增量分析一次（只处理新的提交，通常几秒）。
 *  同一个提交只试一次，失败了不反复跑；项目菜单里能关 */
const autoFresh = () => store.get('autofresh') !== 'off';
let triedHead = '';
async function keepUp(p: any) {
  if (!autoFresh() || p.all || p.exists === false || !state.raw?.head) return;
  const h: string = await api('head', { projectId: p.id }).catch(() => '');
  if (!h || h === state.raw.head || h === triedHead || p !== state.current) return;
  triedHead = h;
  refreshing = false; // reanalyze 自己会换档案
  await reanalyze(p, true);
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
  // 全部项目的名字跟着界面语言
  const name = L('全部项目', 'All projects');
  if (state.current?.all && state.raw?.project && state.raw.project.name !== name) {
    state.raw = { ...state.raw, project: { ...state.raw.project, name } };
  }
  state.model = buildModel(state.raw, { director: false, lang: state.lang, group: groupPref() });
  const m = state.model;
  $('#pname').textContent = m.project || state.current?.name;
  if (state.current?.id) store.set('mark.' + state.current.id, JSON.stringify(petalsOf(m)));
  renderGroup();
  renderPlugs();
}

// ───────────────────────── 接入：提交盖章、跟着发版 ─────────────────────────
// 两项都会改用户的仓库，只在项目菜单里摆两个开关，不在墙上问。开关交给核心的 hook / release install|uninstall
type Plug = { installed: boolean; decided: boolean | null; path: string; github?: boolean; published?: boolean };
type Plugs = { hook: Plug; release: Plug };
let plugs: Plugs | null = null, plugBusy = '';
async function loadPlugs(p: any) {
  try { const r = await api('integrations', { projectId: p.id }); if (p === state.current) plugs = r; }
  catch { if (p === state.current) plugs = null; }
  renderPlugs();
}
/** 名字说清楚它做什么，下面一行说会改到哪里 */
const PLUG_TEXT = {
  hook: () => [L('在提交里记录 AI', 'Note AI in commits'), L('在每次提交中记录工具与行数，写入 Git 提交，无需依赖本地日志', 'Notes tools and line counts in commit messages, recorded in Git commits without relying on local logs')],
  release: () => [L('发版与 PR 自动署名', 'Credits on releases'), L('在 GitHub 发版说明和 PR 评论中附带署名卡，需添加工作流配置', 'Adds credits to GitHub release notes and PR comments via a workflow file you commit')],
};
function plugRow(k: 'hook' | 'release') {
  const on = plugs![k].installed, [title, desc] = PLUG_TEXT[k](), rel = plugs!.release;
  const note = k === 'release' && rel.installed && !rel.published && !plugs!.hook.installed ? L('要先打开上面一项，才有行数可写', 'Turn on the option above first, so there are line counts to write')
    : k === 'release' && rel.github === false && !rel.installed ? L('这个仓库不在 GitHub 上', "This repo isn't on GitHub") : '';
  return `<div class="sw${on ? ' on' : ''}${plugBusy === k ? ' busy' : ''}"><span class="t"><b>${esc(title)}</b><small${note ? ' class="note"' : ''}>${esc(note || desc)}</small></span>
    <button role="switch" aria-checked="${on}" aria-label="${esc(title)}" data-k="${k}" ${plugBusy ? 'disabled' : ''}><i></i></button></div>`;
}
function renderPlugs() {
  const m = document.querySelector<HTMLElement>('#mplugs'); if (!m) return;
  m.hidden = !plugs; if (!plugs) return;
  m.innerHTML = plugRow('hook') + plugRow('release');
  m.querySelectorAll<HTMLElement>('button[data-k]').forEach((b) => (b.onclick = (e) => { e.stopPropagation(); const k = b.dataset.k as 'hook' | 'release'; setPlug(k, !plugs![k].installed); }));
}
async function setPlug(k: 'hook' | 'release', on: boolean, quiet = false) {
  const p = state.current; if (!p || plugBusy) return;
  plugBusy = k; renderPlugs();
  try {
    const r = await api('set_integration', { projectId: p.id, which: k, on, lang: state.lang });
    if (!quiet && r?.summary) toast(String(r.summary), 7000);
  } catch (e: any) { toast(errText(e), 8000); }
  finally { plugBusy = ''; }
  await loadPlugs(p);
}

// ───────────────────────── 分组：按工具 / 按模型 ─────────────────────────
// 没点过就自动挑：一个工具占了绝大部分、下面又有几个模型时按模型分，否则按工具。
// 点了就钉住这个项目的选择。档案里一个模型都没记下就不显示。
type GroupPref = 'auto' | 'tool' | 'model';
const groupPref = (): GroupPref => { const g = state.current ? store.get('group.' + state.current.id) : null; return g === 'tool' || g === 'model' ? g : 'auto'; };
function renderGroup() {
  const el = $('#group');
  if (!state.raw || !hasModels(state.raw)) { el.hidden = true; return; }
  const now = groupOf(state.raw);
  el.hidden = false;
  el.setAttribute('aria-label', L('分组', 'Group by'));
  const name = { tool: L('按工具', 'By tool'), model: L('按模型', 'By model') };
  el.innerHTML = (['tool', 'model'] as const).map((g) => `<button data-g="${g}" class="${g === now ? 'on' : ''}" aria-pressed="${g === now}">${name[g]}</button>`).join('');
  el.querySelectorAll<HTMLElement>('button').forEach((b) => (b.onclick = () => pickGroup(b.dataset.g as GroupPref)));
}
async function pickGroup(g: GroupPref) {
  const p = state.current; if (!p || !state.raw) return;
  if (g === groupOf(state.raw)) return;
  store.set('group.' + p.id, g);
  setGroup(g);
  rebuildModel();
  await paint();
}

// ───────────────────────── 展墙 ─────────────────────────
const curVariant = (k: string) => wallVariant(state.style, state.variants, k);

/** 画过的作品按「档案 + 风格 + 语言 + 作品 + 版式」记住：切回看过的风格、来回换版式都不用再画。
 *  存的是 Promise，同一张正在画的时候墙和放大视图共用一次；按 SVG 总长度限量，最久没用的先丢 */
const cache = new Map<string, Promise<Art | null>>();
const cacheLen = new Map<string, number>();
let cacheRaw: any = null, cacheTotal = 0;
const CACHE_MAX = 32e6; // SVG 字符数
function drawArt(id: string, variant?: string, kit = state.style): Promise<Art | null> {
  if (cacheRaw !== state.raw) { cache.clear(); cacheLen.clear(); cacheTotal = 0; cacheRaw = state.raw; }
  const key = `${kit.id}|${state.lang}|${groupPref()}|${id}|${variant ?? ''}`;
  let hit = cache.get(key);
  if (hit) { cache.delete(key); cache.set(key, hit); return hit; } // 挪到最近用过
  hit = kit.render(state.raw, state.lang, id, variant);
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

/** 上次画出来的尺寸，按「风格 + 作品 + 版式」记着：空墙先按它摆好占位，作品画好落进自己的位置，不用一张张往后挤 */
const dims: Record<string, [number, number]> = (() => { try { return JSON.parse(store.get('dims') ?? '{}'); } catch { return {}; } })();
const dimKey = (id: string, variant?: string) => `${state.style.id}|${id}|${variant ?? ''}`;
function slotFor(b: { id: string; span?: number }, variant?: string) {
  const [w, h] = dims[dimKey(b.id, variant)] ?? [640, 480];
  const g = document.createElement('div');
  g.className = 'tile slot';
  g.dataset.w = String(w); g.dataset.h = String(h);
  g.dataset.span = String(b.span ?? (w / h > 2.2 ? 2 : ''));
  g.style.aspectRatio = `${w} / ${h}`;
  return g;
}
/** 空墙上摆一整面占位；已经摆着（读档案时先摆的）就接着用 */
function slotWall() {
  const board = $('#board');
  const have = [...board.querySelectorAll<HTMLElement>('.slot')];
  if (have.length === tiles().length) return have;
  have.forEach((g) => g.remove());
  const out = tiles().map((b) => board.appendChild(slotFor(b, curVariant(keyOf(b)))));
  layout();
  return out;
}

let wallStale = false;
async function paint() {
  const m = state.model; if (!m) return;
  const st = state.style;
  const token = ++state.token;
  const board = $('#board');
  applyView(); if (roam.ready) roamReset();
  // 在漫游里就不在背后画整面墙（排版线程只有一个，会和漫游抢），切回墙时再画
  wallStale = view() === 'roam';
  if (wallStale) { board.innerHTML = ''; document.body.classList.remove('busy'); return; }
  // 墙上已经有作品（换风格、换语言）：旧的先留着，新的在后台全部画好，再一次原地换上，整面墙只滑动一次；
  // 一张一张换的话，每换一张后面的都要重排，墙会连着抽动十几次。空墙先摆占位，画好一张换掉一张。
  const old = [...board.querySelectorAll('.tile:not(.slot)')] as HTMLElement[];
  const batch = old.length > 0;
  document.body.classList.toggle('busy', batch);
  if (batch) board.querySelectorAll('.slot').forEach((g) => g.remove());
  const slots = batch ? [] : slotWall();
  const fresh: HTMLElement[] = [];
  const keep = new Set<string>();
  for (const [i, b] of tiles().entries()) {
    const k = keyOf(b);
    let variant: string | undefined;
    let a: Art | null = null, preview: Blob | null = null;
    try {
      do {
        variant = curVariant(k);
        a = await drawArt(b.id, variant);
        preview = a && token === state.token ? await st.preview(a) : null;
      } while (token === state.token && variant !== curVariant(k));
    } catch (e) { console.error(b.id, e); }
    if (token !== state.token) return;
    if (!a || !preview) { slots[i]?.remove(); continue; }
    dims[dimKey(b.id, variant)] = [a.w, a.h];
    state.arts.set(k, a); keep.add(k);
    const tile = document.createElement('div');
    tile.className = 'tile';
    tileA11y(tile, b.id);
    tile.dataset.k = k;
    tile.dataset.variant = variant ?? '';
    tile.dataset.w = String(a.w); tile.dataset.h = String(a.h);
    tile.dataset.span = String(b.span ?? (a.w / a.h > 2.2 ? 2 : ''));
    const vs = st.variants(b.id);
    tile.innerHTML = tileHtml(b.id, vs, variant);
    const artEl = tile.querySelector('.art') as HTMLElement;
    mountPreview(artEl, a, preview);
    mountRoamHits(artEl, a);
    wireTile(tile, b.id, k, vs);
    if (batch) { fresh.push(tile); continue; }
    const g = slots[i];
    tile.style.left = g.style.left; tile.style.top = g.style.top; tile.style.width = g.style.width;
    tile.classList.add('placed');
    g.replaceWith(tile);
    layout();
  }
  store.set('dims', JSON.stringify(dims));
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
  return `<div class="name">${esc(state.style.pieceName(id, state.lang, state.raw))}</div>
      <div class="bar glass">${vs.length > 1 ? `<button data-act="prev" title="${L('上一种版式', 'Previous layout')}">‹</button><span class="vname">${esc(state.style.variantName(cur ?? '', state.lang))}</span><button data-act="next" title="${L('下一种版式', 'Next layout')}">›</button>` : ''}<button data-act="export">${L('导出', 'Export')}</button></div>
      <div class="art"></div>`;
}

/** 瀑布流：先确定缩略图尺寸，再排能容纳的栏数；减栏时不能反过来放大作品。
 *  跨栏的作品会在矮的那栏下面留洞，所以每次往后看几张，挑放下去最高、留洞最少的那张先放 */
function layout() {
  const board = $('#board');
  const tiles = [...board.querySelectorAll('.tile')] as HTMLElement[];
  const available = Math.min(innerWidth - 120, 1680);
  const gap = 40;
  // 普通窗口按三栏缩略预览；窄窗口保留可辨认的作品尺寸，剩余空间留在墙的两侧。
  const cw = Math.min(available, 360, Math.max(300, Math.floor((available - gap * 2) / 3)));
  const cols = Math.max(1, Math.min(4, Math.floor((available + gap) / (cw + gap))));
  const W = cw * cols + gap * (cols - 1);
  const hs = new Array(cols).fill(0);
  const at = (t: HTMLElement, span: number) => {
    const a = { w: Number(t.dataset.w), h: Number(t.dataset.h) };
    const full = cw * span + gap * (span - 1);
    const w = Math.min(full, a.w * (a.w < 500 ? 1.15 : 1));
    let best = 0, y = Infinity;
    for (let c = 0; c + span <= cols; c++) { const v = Math.max(...hs.slice(c, c + span)); if (v < y - 1) { y = v; best = c; } }
    let hole = 0; for (let c = best; c < best + span; c++) hole += y - hs[c];
    return { span, full, w, h: (w / a.w) * a.h, best, y, hole };
  };
  const fit = (t: HTMLElement) => {
    const set = Number(t.dataset.span);
    const small = Number(t.dataset.w) < 500;
    // 跨栏只用于三栏以上的宽幅作品；不按临时空洞改变尺寸，避免重排时忽大忽小。
    const span = cols >= 3 && !small ? set || (Number(t.dataset.w) / Number(t.dataset.h) > 1.7 ? 2 : 1) : 1;
    return at(t, Math.min(cols, span));
  };
  const queue = tiles.slice();
  while (queue.length) {
    let pick = 0, f = fit(queue[0]), score = f.y + f.hole;
    for (let j = 1; j < Math.min(4, queue.length); j++) {
      const g = fit(queue[j]), sc = g.y + g.hole + j * 80; // 越往后越不愿意提前，尽量保持原来的顺序
      if (sc < score - 1) { pick = j; f = g; score = sc; }
    }
    const t = queue.splice(pick, 1)[0];
    const x = f.best * (cw + gap) + (f.full - f.w) / 2;
    t.style.left = `${x}px`; t.style.top = `${f.y}px`; t.style.width = `${f.w}px`;
    for (let c = f.best; c < f.best + f.span; c++) hs[c] = f.y + f.h + gap + 22;
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
      // 从现在选中的算起，连点两下就走两步；名字和图一起换。
      const i = vs.indexOf(requestedVariant(k) ?? '');
      const v = vs[(i + (act === 'next' ? 1 : vs.length - 1)) % vs.length];
      return changeVariant(id, k, v);
    }
    if (act === 'export') toggleFormats(tile, id, (f) => doExport(f, id, k));
  });
  const artEl = tile.querySelector('.art') as HTMLElement;
  artEl.addEventListener('pointerdown', (e) => startDrag(e, tile, id, k));
  tile.addEventListener('keydown', (e) => { if (e.target === tile && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); openFocus(id, k); } });
}
/** 键盘也能用：Tab 走到每一张，回车放大 */
function tileA11y(tile: HTMLElement, id: string) {
  tile.tabIndex = 0;
  tile.setAttribute('role', 'button');
  tile.setAttribute('aria-label', `${state.style.pieceName(id, state.lang, state.raw)} · ${L('回车放大', 'press Enter to enlarge')}`);
}
function applyVariant(id: string, k: string, variant: string) {
  if (!selectWallVariant(state.style, state.variants, k, variant).length) return;
  if (view() === 'roam') { wallStale = true; return; }
  // 整墙正在换风格或语言时，旧批次里已经画好的版式也要作废。
  if (document.body.classList.contains('busy')) void paint();
  else void repaintPiece(id);
}
const variantRequests = new Map<string, { variant: string; valid(): boolean }>();
const requestedVariant = (k: string) => {
  const request = variantRequests.get(k);
  return request?.valid() ? request.variant : curVariant(k);
};
/** 先确认目标版式能生成；失败时保留原图和原选择，连续点击仍按最后一次意图走。 */
async function changeVariant(id: string, k: string, variant: string) {
  const kit = state.style, raw = state.raw, lang = state.lang, project = state.openSeq, group = groupPref();
  const request = { variant, valid: () => state.style === kit && state.raw === raw && state.lang === lang && state.openSeq === project && groupPref() === group };
  variantRequests.set(k, request);
  const current = () => variantRequests.get(k) === request && request.valid();
  try {
    const a = await drawArt(id, variant, kit);
    if (!current()) return;
    if (!a) throw new Error(L('当前档案无法生成这个版式', 'This layout is unavailable for the current project'));
    await kit.preview(a);
    if (current()) applyVariant(id, k, variant);
  } catch (e) { if (current()) toast(errText(e), 8000); }
  finally { if (variantRequests.get(k) === request) variantRequests.delete(k); }
}
/** 同一作品的版式一起换好，再一次落位；连点只采用最后一次，避免交换到一半出现重复。 */
const tileSeq = new Map<string, number>();
async function repaintPiece(id: string) {
  const token = state.token, kit = state.style, seq = (tileSeq.get(id) ?? 0) + 1;
  tileSeq.set(id, seq);
  try {
    const fresh = await Promise.all(tiles().filter((b) => b.id === id && document.querySelector(`#board .tile[data-k="${keyOf(b)}"]`)).map(async (b) => {
      const k = keyOf(b), variant = curVariant(k), a = await drawArt(id, variant);
      return { b, k, variant, a, preview: a && token === state.token && tileSeq.get(id) === seq ? await kit.preview(a) : null };
    }));
    if (token !== state.token || tileSeq.get(id) !== seq) return;
    for (const { b, k, variant, a, preview } of fresh) {
      const tile = document.querySelector(`#board .tile[data-k="${k}"]`) as HTMLElement | null;
      if (!tile || !a || !preview) continue;
      state.arts.set(k, a);
      dims[dimKey(id, variant)] = [a.w, a.h];
      tile.dataset.variant = variant ?? '';
      tile.dataset.w = String(a.w); tile.dataset.h = String(a.h);
      tile.dataset.span = String(b.span ?? (a.w / a.h > 2.2 ? 2 : ''));
      const name = tile.querySelector('.vname'); if (name) name.textContent = kit.variantName(variant ?? '', state.lang);
      const artEl = tile.querySelector('.art') as HTMLElement;
      releasePreview(artEl); artEl.replaceChildren();
      mountPreview(artEl, a, preview); mountRoamHits(artEl, a);
    }
    layout();
  } catch (e) { toast(errText(e), 8000); }
}

// ───────────────────────── 导出 ─────────────────────────
// 墙上、漫游、放大视图用同一个导出菜单：格式一列，下面一排尺寸；记住上次用的格式和尺寸
const closeFormats = () => document.querySelectorAll('.fmts').forEach((x) => { x.parentElement!.classList.remove('menuopen'); x.remove(); });
function toggleFormats(host: HTMLElement, id: string, run: (fmt: string) => void, kit = state.style) {
  const open = host.querySelector(':scope > .fmts');
  closeFormats();
  if (open) return;
  const box = document.createElement('div');
  box.className = 'fmts glass';
  const last = store.get('fmt');
  box.innerHTML = exportsFor(id, kit).map((f) => `<button data-f="${f.id}" class="${f.id === last ? 'last' : ''}">${f.name}</button>`).join('')
    + `<div class="szs" role="group" aria-label="${L('尺寸', 'Size')}">${sizeButtons()}</div>`;
  box.onclick = (e) => {
    e.stopPropagation();
    const b = (e.target as HTMLElement).closest('button'); if (!b) return;
    if (b.dataset.s) { setSize(b.dataset.s); return; }
    closeFormats(); store.set('fmt', b.dataset.f!); run(b.dataset.f!);
  };
  host.appendChild(box); host.classList.add('menuopen');
  setTimeout(() => document.addEventListener('click', function off() { box.remove(); host.classList.remove('menuopen'); document.removeEventListener('click', off); }), 0);
}
/** 嵌入组件还没跟上原版，先不给；GIF 只给会动的作品 */
const EXPORTS = FORMATS.filter((f) => f.id !== 'embed');
const exportsFor = (id: string, kit = state.style) => EXPORTS.filter((f) => f.id !== 'gif' || kit.animated(id));
const SIZE_NAME: Record<string, [string, string]> = { native: ['原尺寸', 'Native'], og: ['横图', 'Wide'], square: ['方图', 'Square'], portrait: ['竖图', 'Portrait'], story: ['竖屏', 'Story'] };
const sizeButtons = () => SIZES.filter((s) => s.id !== 'a4').map((s) => `<button data-s="${s.id}" class="${s.id === state.size ? 'on' : ''}" title="${s.w ? `${s.w}×${s.h}` : ''}">${esc(SIZE_NAME[s.id] ? L(...SIZE_NAME[s.id]) : s.id)}</button>`).join('');
function setSize(id: string) {
  state.size = id; store.set('size', id);
  document.querySelectorAll<HTMLElement>('[data-s]').forEach((b) => { if (b.closest('.szs, #sizes, #defsize')) b.classList.toggle('on', b.dataset.s === id); });
}

/** 默认导出墙上这一张；放大视图里试别的风格时，把那一套和画好的图传进来 */
async function doExport(fmtId: string, id: string, k: string, o: { kit?: Kit; art?: Art; variant?: string } = {}) {
  const kit = o.kit ?? state.style;
  const m = state.model!;
  const variant = o.kit ? o.variant : curVariant(k);
  const base = `${m.project || 'credits'}-${id}${variant && variant !== 'original' ? '-' + variant : ''}-${kit.id}`;
  try {
    if (fmtId === 'md') {
      const text = mergeCreditsMd(null, creditsBlock(m));
      try { await navigator.clipboard.writeText(text); return toast(L('CREDITS.md 区块已复制', 'CREDITS.md block copied')); }
      catch { return toast(L('没能写进剪贴板', 'Could not copy to the clipboard'), 8000, { acts: [{ label: L('存成文件', 'Save as a file'), run: () => saveBytes('CREDITS.md', new TextEncoder().encode(text), 'text/markdown').catch((e) => toast(errText(e), 5000)) }] }); }
    }
    toast(fmtId === 'gif' ? L('正在逐帧生成 GIF…', 'Rendering GIF frames…') : L('正在导出…', 'Exporting…'), 60000, { spin: true });
    const a = o.art ?? await drawArt(id, variant, kit);
    if (!a) throw new Error(L('当前档案无法生成这个版式', 'This layout is unavailable for the current project'));
    await new Promise((r) => setTimeout(r, 30));
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
  dock.innerHTML = EXPORTS.filter((f) => f.id !== 'gif').map((f) => `<button data-f="${f.id}">${f.name}</button>`).join('');
  $('#hint').textContent = L('松手导出', 'Drop to export');
  const sizes = $('#sizes');
  sizes.innerHTML = sizeButtons();
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
      ghost.innerHTML = '';
      ghost.style.aspectRatio = `${a.w} / ${a.h}`;
      void state.style.preview(a).then((p) => { if (dragging) mountPreview(ghost, a, p); });
      ghost.hidden = false;
      ghost.animate([{ scale: String(r0.width / w) }, { scale: '1' }], { duration: 220, easing: 'cubic-bezier(.2,.8,.2,1)' });
      document.querySelector('.ct-tip')?.classList.remove('on');
    }
    if (!dragging) return;
    ghost.style.translate = `${ev.clientX - ghost.offsetWidth * fx}px ${ev.clientY - ghost.offsetHeight * fy}px`; // 用单独的 translate，缩放才不会连位置一起缩
    const tb = nearest($('#targets'), ev.clientX, ev.clientY);
    const sz = tb ? null : nearest($('#sizes'), ev.clientX, ev.clientY);
    if (sz && sz.dataset.s !== state.size) setSize(sz.dataset.s!);
    if (tb !== over) { over?.classList.remove('over'); over = tb; over?.classList.add('over'); }
    // 进了导出坞就缩小让出按钮；整块坞都算，划过按钮之间的缝不会忽大忽小
    ghost.classList.toggle('small', inside($('#dock'), ev.clientX, ev.clientY, 16));
  };
  const up = (ev: PointerEvent) => {
    removeEventListener('pointermove', move); removeEventListener('pointerup', up); removeEventListener('pointercancel', up);
    if (!dragging) return ev.type === 'pointerup' ? openFocus(id, k) : undefined;
    if (ev.type === 'pointercancel') { over?.classList.remove('over'); over = null; }
    document.body.classList.remove('dragging');
    dragging = false; releasePreview(ghost);
    ghost.hidden = true; ghost.innerHTML = ''; ghost.classList.remove('small');
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
  if ($('#dots').querySelector('.styles')) return closeStyles();
  closeFormats();
}
const closeBtn = () => `<button class="closer glass" data-act="close" aria-label="${L('关闭', 'Close')}" title="${L('关闭', 'Close')} (Esc)"><svg width="12" height="12" viewBox="0 0 12 12"><path d="M2.5 2.5l7 7M9.5 2.5l-7 7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg></button>`;

function openFocus(id: string, k: string) {
  const el = $('#focus');
  // 放大视图里可以只给这一件换风格试试：kit 是正在看的那一套，墙上的风格不动
  let kit = state.style, art: Art | null = null, preview: Blob | null = null, seq = 0, open = true;
  const tried: Record<string, string> = {};
  const variantNow = () => kit === state.style ? curVariant(k) : tried[kit.id] ?? kit.current(id);
  const setVariant = async (v: string) => { if (kit === state.style) await changeVariant(id, k, v); else tried[kit.id] = v; };
  const keyList = () => {
    const wall = [...document.querySelectorAll<HTMLElement>('#board .tile:not(.slot)')].map((t) => t.dataset.k!);
    return wall.length ? wall : tiles().map(keyOf); // 墙在漫游里没画，就按作品顺序走
  };
  const step = (d: number) => {
    const all = keyList(), next = all[all.indexOf(k) + d];
    if (!next) return;
    close(false); openFocus(next.split('#')[0], next);
    document.querySelector(`.tile[data-k="${next}"]`)?.scrollIntoView({ block: 'nearest' });
  };
  const chrome = () => {
    const vs = kit.variants(id), v = variantNow(), all = keyList(), at = all.indexOf(k);
    const multiModel = !!state.raw && resolveGroup(state.raw, 'auto') === 'model';
    el.innerHTML = `${closeBtn()}
      <div class="fhead"><b>${esc(kit.pieceName(id, state.lang, state.raw))}</b></div>
      <button class="nav prev glass" data-act="prev" ${at <= 0 ? 'disabled' : ''} title="${L('上一张', 'Previous')} (↑)" aria-label="${L('上一张', 'Previous')}"><svg width="14" height="14" viewBox="0 0 14 14"><path d="M8.5 3 4.5 7l4 4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg></button>
      <button class="nav next glass" data-act="next" ${at < 0 || at >= all.length - 1 ? 'disabled' : ''} title="${L('下一张', 'Next')} (↓)" aria-label="${L('下一张', 'Next')}"><svg width="14" height="14" viewBox="0 0 14 14"><path d="M5.5 3l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg></button>
      <div class="big"><div class="art"></div></div>
      <div class="strip glass">
        ${vs.length > 1 ? `<div class="vs" title="${L('版式', 'Layout')} (← →)">${vs.map((x) => { const mm = multiModel && MODEL_VARIANTS[id]?.includes(x); return `<button data-v="${x}" class="${x === v ? 'on' : ''}${mm ? ' mm' : ''}">${esc(kit.variantName(x, state.lang))}</button>`; }).join('')}</div><span class="sep"></span>` : ''}
        <div class="pop"><button class="kit" data-act="kits"><svg viewBox="0 0 24 24">${kit.dot}</svg>${esc(kit.name(state.lang))}</button></div>
        ${kit !== state.style ? `<button class="apply" data-act="apply">${L('设为全局风格', 'Use for all cards')}</button>` : ''}
        <span class="sep"></span>
        <div class="pop"><button class="xp" data-act="export">${L('导出', 'Export')}</button></div>
      </div>`;
    if (art && preview) { mountPreview(el.querySelector('.big .art') as HTMLElement, art, preview); place(art); } // 换风格时旧图先留着，新的画好再换
  };
  const place = (a: Art) => { (el.querySelector('.big') as HTMLElement).style.width = `${Math.max(320, Math.min(a.w, 1100, ((innerHeight - 240) * a.w) / a.h))}px`; };
  // 快速连点几个版式：按钮立刻跟上，图只认最后一次（先点的画得慢，回来也不会盖掉后点的）
  const draw = async () => {
    const my = ++seq, v = variantNow(), k0 = kit;
    el.querySelectorAll<HTMLElement>('.strip button[data-v]').forEach((b) => b.classList.toggle('on', b.dataset.v === v));
    el.querySelector('.vs .on')?.scrollIntoView({ block: 'nearest', inline: 'nearest' }); // 版式多时这一排会横着滚
    el.querySelector('.big')?.classList.add('loading');
    const a = await drawArt(id, v, k0);
    if (!a || my !== seq || !open) return;
    const p = await k0.preview(a);
    if (my !== seq || !open) return;
    art = a; preview = p;
    if (k0 === state.style) state.arts.set(k, a);
    const big = el.querySelector('.big') as HTMLElement, artEl = el.querySelector('.big .art') as HTMLElement;
    big.classList.remove('loading');
    releasePreview(artEl); artEl.replaceChildren();
    mountPreview(artEl, a, p);
    place(a);
    mountRoamHits(artEl, a);
  };
  const pickKit = (host: HTMLElement) => {
    const open = host.querySelector(':scope > .kits'); closeFormats(); el.querySelector('.kits')?.remove();
    if (open) return;
    const box = document.createElement('div');
    box.className = 'kits glass';
    box.innerHTML = KITS.map((t, j) => `${j === CLASSIC_COUNT ? '<i class="divider"></i>' : ''}<button data-s="${t.id}" class="${t === kit ? 'on' : ''}" ${t.board.some((b) => b.id === id) ? '' : 'disabled'}><svg viewBox="0 0 24 24">${t.dot}</svg>${esc(t.name(state.lang))}</button>`).join('');
    host.appendChild(box);
    // 从风格按钮正上方弹出，左边和按钮对齐；碰到窗口边就往里收
    const r = box.getBoundingClientRect(), over = Math.max(0, r.right - (innerWidth - 16)) - Math.max(0, 16 - r.left);
    if (over) box.style.left = `${-over}px`;
  };
  chrome(); draw();
  el.hidden = false;
  el.onclick = (e) => {
    const b = (e.target as HTMLElement).closest('button') as HTMLElement | null;
    if (!b) { el.querySelector('.kits')?.remove(); closeFormats(); if (e.target === el) close(); return; }
    e.stopPropagation();
    const act = b.dataset.act;
    if (act === 'close') return close();
    if (act === 'prev' || act === 'next') return step(act === 'next' ? 1 : -1);
    if (b.closest('.kits') && b.dataset.s) { kit = kitById(b.dataset.s)!; chrome(); draw(); return; }
    if (act === 'kits') return pickKit(b.parentElement!);
    if (act === 'apply') {
      const v = variantNow(), t = kit;
      close(false);
      state.style = t; store.set('theme', t.id); state.variants = {}; renderDots();
      const first = tiles().find((b) => b.id === id);
      if (first && v) selectWallVariant(t, state.variants, keyOf(first), v);
      return void paint();
    }
    if (act === 'export') { el.querySelector('.kits')?.remove(); return toggleFormats(b.parentElement!, id, (f) => { if (art) doExport(f, id, k, kit === state.style ? {} : { kit, art, variant: variantNow() }); }, kit); }
    if (b.dataset.v) void setVariant(b.dataset.v).then(() => { if (open) draw(); });
  };
  // 版式已同步到墙上；关闭后键盘焦点回到刚才的位置。
  const close = (refocus = true) => {
    open = false; el.hidden = true; el.innerHTML = ''; closeFocus = null; focusKeys = null;
    document.querySelector('.ct-tip')?.classList.remove('on');
    if (refocus) (document.querySelector(`.tile[data-k="${k}"]`) as HTMLElement | null)?.focus({ preventScroll: true });
  };
  closeFocus = close;
  focusKeys = (e) => {
    if (e.metaKey || e.ctrlKey || e.altKey) return false;
    const vs = kit.variants(id);
    if ((e.key === 'ArrowLeft' || e.key === 'ArrowRight') && vs.length > 1) {
      const j = vs.indexOf((kit === state.style ? requestedVariant(k) : variantNow()) ?? '');
      void setVariant(vs[(j + (e.key === 'ArrowRight' ? 1 : vs.length - 1)) % vs.length]).then(() => { if (open) draw(); });
      return true;
    }
    if (e.key === 'ArrowUp' || e.key === 'ArrowDown') { step(e.key === 'ArrowDown' ? 1 : -1); return true; }
    return false;
  };
}

// ───────────────────────── 漫游：上下左右没有尽头的画布 ─────────────────────────
// 墙之外的另一种看法。每一栏自己往上往下无限延伸，栏和栏也无限往左右排；作品按顺序轮着出现，
// 每轮一圈就换下一种版式，所以走得越远，看到的是同一件作品的别的画法，而不是原样复制。
// 拖动（或滚轮、触控板）移动画布；点一下放大，悬停点「导出」。只把视口附近的作品放进页面。
type RItem = { r: number; y: number; h: number; w: number; id: string; k: string; v?: string; a: Art; p: Blob };
type RCol = { items: RItem[]; busy: boolean; next: number; prev: number };
const roam = { x: 0, y: 0, cols: new Map<number, RCol>(), els: new Map<string, HTMLElement>(), gen: 0, ready: false };
let roamViewport: RoamViewport | undefined;
const CAP = 30, RGAP = 56, STEP = 5;
const view = () => (store.get('view') === 'roam' ? 'roam' : 'wall');
const rcw = () => Math.round(Math.max(380, Math.min(520, innerWidth / 3.1)));
const pitch = () => rcw() + RGAP;
const mod = (a: number, n: number) => ((a % n) + n) % n;
/** 每一栏起点错开一点，栏和栏的作品不会排成一行一行 */
const colOff = (X: number) => mod(Math.imul(X ^ 0x5bd1e995, 0x27d4eb2d) >>> 0, 997) / 997 * 420;

/** 第 X 栏第 r 个是哪件作品、哪个版式 */
function slot(X: number, r: number) {
  const list = state.style.board, n = list.length;
  const v = X + r * STEP, i = mod(v, n), lap = Math.floor(v / n);
  const b = list[i], k = keyOf({ ...b, variant: state.style.current(b.id, b.variant) });
  const vs = state.style.variants(b.id);
  const base = curVariant(k);
  const j = vs.indexOf(base ?? '');
  return { b, k, v: vs.length > 1 ? vs[mod((j < 0 ? 0 : j) + lap, vs.length)] : base };
}
async function rItem(X: number, r: number): Promise<RItem | null> {
  const s = slot(X, r), gen = roam.gen, kit = state.style;
  let a: Art | null = null, p: Blob | null = null;
  try { a = await drawArt(s.b.id, s.v); if (a && gen === roam.gen) p = await kit.preview(a); } catch (e) { console.error(s.b.id, e); }
  if (gen !== roam.gen) return null;
  if (!a || !p) return null;
  const cw = rcw(), w = Math.min(cw, a.w < 500 ? a.w * 1.15 : cw);
  return { r, y: 0, w, h: (w / a.w) * a.h, id: s.b.id, k: s.k, v: s.v, a, p };
}
/** 让第 X 栏盖住 [top, bottom]；一栏同时只长一处，长完再看一遍 */
async function grow(X: number, top: number, bottom: number) {
  let col = roam.cols.get(X);
  if (!col) { col = { items: [], busy: false, next: 0, prev: -1 }; roam.cols.set(X, col); }
  if (col.busy) return;
  col.busy = true;
  const gen = roam.gen;
  const before = col.items.length;
  try {
    let guard = 0;
    while (gen === roam.gen && guard++ < 40) {
      const it = col.items;
      if (!it.length) {
        const f = await rItem(X, col.next++); if (gen !== roam.gen) return;
        if (!f) continue;
        f.y = colOff(X); it.push(f); continue;
      }
      const last = it[it.length - 1], first = it[0];
      if (last.y + last.h < bottom) {
        const f = await rItem(X, col.next++); if (gen !== roam.gen) return;
        if (!f) continue; // 跳过空作品，已显示作品的行号保持不变
        f.y = last.y + last.h + RGAP + CAP; it.push(f);
      } else if (first.y - CAP > top) {
        const f = await rItem(X, col.prev--); if (gen !== roam.gen) return;
        if (!f) continue;
        f.y = first.y - CAP - RGAP - f.h; it.unshift(f);
      } else break;
      place();
    }
  } finally { col.busy = false; }
  // 没有新增作品时不自动重试，否则空数据会形成永不让出界面线程的 Promise 循环。
  if (gen === roam.gen && col.items.length > before) place();
}
/** 移动、补图和惯性可能同时到达，同一帧只整理一次画布。 */
let placeFrame = 0;
function place() {
  if (!roam.ready || view() !== 'roam' || placeFrame) return;
  placeFrame = requestAnimationFrame(() => { placeFrame = 0; placeNow(); });
}
/** 预览文字在排版线程转路径，浏览器以独立图片绘制，避免把整张矢量图挂进页面。 */
const previewURLs = new WeakMap<HTMLElement, string>();
function releasePreview(root: HTMLElement) {
  const url = previewURLs.get(root); if (url) { URL.revokeObjectURL(url); previewURLs.delete(root); }
}
function mountPreview(root: HTMLElement, art: Art, preview: Blob) {
  releasePreview(root);
  const image = document.createElement('img');
  image.width = art.w; image.height = art.h; image.alt = art.title;
  image.decoding = 'async'; image.draggable = false;
  const url = URL.createObjectURL(preview); previewURLs.set(root, url);
  image.onload = image.onerror = () => { if (previewURLs.get(root) === url) releasePreview(root); };
  image.src = url; root.appendChild(image);
}
/** 只给正在查看的卡片建立热区，离开即卸载；拖动时不新建交互节点。 */
const previewHits = new WeakMap<HTMLElement, () => void>();
function mountRoamHits(root: HTMLElement, art: Art) {
  previewHits.get(root)?.();
  let unmount: (() => void) | undefined;
  const enter = () => { if (!unmount && !root.closest('#roam.moving')) unmount = mountHits(root, art.hits, art.w, art.h); };
  const leave = () => { unmount?.(); unmount = undefined; };
  root.addEventListener('pointerenter', enter);
  root.addEventListener('pointerleave', leave);
  previewHits.set(root, () => { leave(); root.removeEventListener('pointerenter', enter); root.removeEventListener('pointerleave', leave); });
  if (root.matches(':hover')) enter();
}
/** 屏内优先，其次是离视口最近的预加载卡片；用布局数据计算，不读取 DOM 尺寸。 */
function previewPriority(el: HTMLElement, t: RItem) {
  return () => {
    const left = parseFloat(el.style.left);
    return Math.max(0, roam.x - left - t.w, left - roam.x - innerWidth)
      + Math.max(0, roam.y - t.y - t.h, t.y - roam.y - innerHeight);
  };
}
/** 只把视口附近的作品放进页面，远的拿掉；缺的让那一栏去长 */
function placeNow() {
  if (!roam.ready || !state.model || view() !== 'roam') return;
  const plane = $('#roam .plane');
  const P = pitch(), cw = rcw(), m = 700;
  const top = roam.y - m, bottom = roam.y + innerHeight + m;
  const want = new Set<string>();
  for (let X = Math.floor((roam.x - m) / P); X <= Math.floor((roam.x + innerWidth + m) / P); X++) {
    const col = roam.cols.get(X);
    const it = col?.items ?? [];
    if (!it.length || it[0].y - CAP > top || it[it.length - 1].y + it[it.length - 1].h < bottom) grow(X, top, bottom);
    for (const t of it) {
      if (t.y + t.h < top || t.y - CAP > bottom) continue;
      const key = `${X}:${t.r}`;
      want.add(key);
      if (roam.els.has(key)) continue;
      const el = document.createElement('div');
      el.className = 'tile rt';
      el.style.left = `${X * P + (cw - t.w) / 2}px`; el.style.top = `${t.y}px`; el.style.width = `${t.w}px`;
      el.innerHTML = `<div class="name">${esc(state.style.pieceName(t.id, state.lang, state.raw))}${t.v && state.style.variants(t.id).length > 1 ? `<span class="vn">${esc(state.style.variantName(t.v, state.lang))}</span>` : ''}</div>
        <div class="bar glass"><button data-act="export">${L('导出', 'Export')}</button></div><div class="art"></div>`;
      const artEl = el.querySelector('.art') as HTMLElement;
      mountRoamPreview(artEl, t.a, t.p, Math.ceil(t.w * devicePixelRatio), previewPriority(el, t));
      mountRoamHits(artEl, t.a);
      (el as any)._item = t;
      plane.appendChild(el);
      roam.els.set(key, el);
    }
  }
  for (const [key, el] of roam.els) if (!want.has(key)) { releaseRoamPreview(el.querySelector('.art') as HTMLElement); el.remove(); roam.els.delete(key); }
}
/** 全屏和调整窗口时原地重排，保留已有图像和视口中心，避免清空后再画一整屏。 */
function resizeRoam(oldWidth: number, oldViewportWidth: number, oldViewportHeight: number) {
  const centerX = (roam.x + oldViewportWidth / 2) / (oldWidth + RGAP);
  const centerY = roam.y + oldViewportHeight / 2;
  const centerColumn = roam.cols.get(Math.floor(centerX));
  const anchor = centerColumn?.items.find((t) => t.y <= centerY && t.y + t.h + RGAP + CAP >= centerY);
  const fraction = anchor ? Math.min(1, (centerY - anchor.y) / anchor.h) : 0;
  const gap = anchor ? Math.max(0, centerY - anchor.y - anchor.h) : 0; // 图间距固定，不能随列宽缩放
  const cw = rcw(), P = pitch();
  if (cw !== oldWidth) for (const [X, col] of roam.cols) {
    for (const t of col.items) { t.w = Math.min(cw, t.a.w < 500 ? t.a.w * 1.15 : cw); t.h = t.w / t.a.w * t.a.h; }
    const pivot = Math.max(0, col.items.findIndex((t) => t.r >= 0));
    if (!col.items.length) continue;
    col.items[pivot].y = colOff(X);
    for (let i = pivot + 1; i < col.items.length; i++) col.items[i].y = col.items[i - 1].y + col.items[i - 1].h + RGAP + CAP;
    for (let i = pivot - 1; i >= 0; i--) col.items[i].y = col.items[i + 1].y - RGAP - CAP - col.items[i].h;
    for (const t of col.items) {
      const el = roam.els.get(`${X}:${t.r}`); if (!el) continue;
      el.style.left = `${X * P + (cw - t.w) / 2}px`; el.style.top = `${t.y}px`; el.style.width = `${t.w}px`;
    }
  }
  roam.x = centerX * P - innerWidth / 2;
  roam.y = (anchor ? anchor.y + fraction * anchor.h + gap : centerY * cw / oldWidth) - innerHeight / 2;
  for (const el of roam.els.values()) {
    const t = (el as any)._item as RItem, art = el.querySelector('.art') as HTMLElement;
    const width = Math.ceil(t.w * devicePixelRatio);
    if (art.querySelector('canvas')?.width !== width) mountRoamPreview(art, t.a, t.p, width, previewPriority(el, t));
  }
  roamViewport?.moveTo(roam.x, roam.y);
  place();
}

/** 墙上那件作品借用这一张的版式和画好的图：放大、导出都照这一张来 */
function adopt(t: RItem) { if (t.v) applyVariant(t.id, t.k, t.v); state.arts.set(t.k, t.a); }
/** 回到起点：第一栏贴着左边居中排开，最上面让出顶栏 */
function home(): [number, number] {
  const P = pitch(), n = Math.max(1, Math.floor((innerWidth + RGAP) / P));
  return [-Math.round((innerWidth - (n * P - RGAP)) / 2), -96];
}
function clearRoamElements() {
  roamViewport?.stop();
  cancelAnimationFrame(placeFrame); placeFrame = 0;
  for (const el of roam.els.values()) releaseRoamPreview(el.querySelector('.art') as HTMLElement);
  roam.els.clear();
  $('#roam .plane').innerHTML = '';
}
function roamReset() {
  clearRoamElements();
  roam.gen++;
  roam.cols.clear();
  if (view() === 'roam' && state.model) place();
}
function glide(tx: number, ty: number) {
  roamViewport?.moveTo(tx, ty, 'smooth');
}
function wireRoam() {
  const el = $('#roam');
  let movingTimer = 0;
  let drag: { x: number; y: number; px: number; py: number; moved: boolean; t: number; hist: [number, number, number][] } | null = null;
  const moving = () => {
    el.classList.add('moving');
    clearTimeout(movingTimer);
    movingTimer = window.setTimeout(() => { if (!drag?.moved) el.classList.remove('moving'); }, 120);
  };
  roamViewport = new RoamViewport(el, (x, y) => { roam.x = x; roam.y = y; moving(); place(); });
  el.addEventListener('pointerdown', (e) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest('button, .fmts')) return;
    roamViewport!.stop();
    drag = { x: e.clientX, y: e.clientY, px: roam.x, py: roam.y, moved: false, t: performance.now(), hist: [] };
    el.setPointerCapture(e.pointerId);
  });
  el.addEventListener('pointermove', (e) => {
    if (!drag) return;
    const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
    if (!drag.moved && Math.hypot(dx, dy) > 5) { drag.moved = true; el.classList.add('panning'); closeFormats(); }
    if (!drag.moved) return;
    roamViewport!.moveTo(drag.px - dx, drag.py - dy);
    drag.hist.push([performance.now(), roam.x, roam.y]); if (drag.hist.length > 6) drag.hist.shift();
  });
  const up = (e: PointerEvent) => {
    if (!drag) return;
    const d = drag; drag = null;
    el.classList.remove('panning', 'moving');
    if (!d.moved) {
      if (e.type !== 'pointerup') return;
      // 没拖动就是点了一下：放大这一张
      const tile = document.elementsFromPoint(e.clientX, e.clientY).find((x) => x.classList.contains('rt')) as HTMLElement | undefined;
      const t = (tile as any)?._item as RItem | undefined;
      if (t) { adopt(t); openFocus(t.id, t.k); }
      return;
    }
    // 松手后按最后一小段的速度滑一会儿
    const h = d.hist; if (e.type !== 'pointerup' || h.length < 2) return;
    const [t1, x1, y1] = h[0], [t2, x2, y2] = h[h.length - 1], dt = Math.max(16, t2 - t1);
    if (performance.now() - t2 > 80) return;
    // 保留原来每 16ms 衰减到 94% 的滑行距离。
    const distance = 16 * 0.94 / (1 - 0.94);
    glide(roam.x + (x2 - x1) / dt * distance, roam.y + (y2 - y1) / dt * distance);
  };
  el.addEventListener('pointerup', up); el.addEventListener('pointercancel', up);
  el.addEventListener('wheel', (e) => {
    e.preventDefault();
    const scale = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? innerHeight : 1;
    // 同时保留两个方向的输入，触控板可斜向移动；普通滚轮按 Shift 横移。
    const dx = e.shiftKey && !e.deltaX ? e.deltaY : e.deltaX;
    const dy = e.shiftKey && !e.deltaX ? 0 : e.deltaY;
    roamViewport!.moveTo(roam.x + dx * scale, roam.y + dy * scale);
  }, {passive:false});
  el.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest('button[data-act="export"]'); if (!b) return;
    e.stopPropagation();
    const tile = b.closest('.rt') as HTMLElement, t = (tile as any)._item as RItem;
    adopt(t); toggleFormats(tile, t.id, (f) => doExport(f, t.id, t.k));
  });
  $('#home').onclick = () => glide(...home());
  $('#view').onclick = (e) => {
    const b = (e.target as HTMLElement).closest('button') as HTMLElement | null; if (!b) return;
    store.set('view', b.dataset.v!); applyView();
    if (view() === 'wall' && wallStale) void paint();
  };
  addEventListener('keydown', (e) => {
    if (view() !== 'roam' || !document.body.classList.contains('roam') || closeFocus || !$('#sheet').hidden || !$('#menu').hidden) return;
    if ((e.target as HTMLElement).closest('input, textarea')) return;
    const d = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }[e.key];
    if (d) { e.preventDefault(); glide(roam.x + d[0] * pitch(), roam.y + d[1] * innerHeight * 0.6); }
    if (e.key === 'Home' || e.key === '0') glide(...home());
  });
}
/** 墙 / 漫游。没有作品（空状态）时总是墙，空状态的说明在墙上 */
function applyView() {
  const on = view() === 'roam' && !!state.model && $('#empty').hidden;
  document.querySelectorAll<HTMLElement>('#view button').forEach((b) => b.classList.toggle('on', b.dataset.v === view()));
  // 没有作品（空作品库、第一次打开）：只留语言和设置，风格条也收起来
  document.body.classList.toggle('bare', !$('#empty').hidden || !$('#welcome').hidden);
  const was = document.body.classList.contains('roam');
  document.body.classList.toggle('roam', on);
  if (!on && was) clearRoamElements();
  $('#roam').hidden = !on; $('#home').hidden = !on;
  if (on && !roam.ready) { roam.ready = true; [roam.x, roam.y] = home(); roamViewport?.moveTo(roam.x, roam.y); roamReset(); }
  else if (on && !was) { roamViewport?.moveTo(roam.x, roam.y); place(); }
}

// ───────────────────────── 风格 ─────────────────────────
// 左下一颗按钮写着现在的风格；‹ › 一步一换，点名字列出全部（原版主题和新风格之间一道细线）
const STEP_SVG = (d: string) => `<svg width="12" height="12" viewBox="0 0 12 12"><path d="${d}" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
function renderDots() {
  const nav = $('#dots'), t = state.style;
  nav.innerHTML = `<button class="step" data-d="-1">${STEP_SVG('M7.5 2.5 4 6l3.5 3.5')}</button>`
    + `<button class="cur" id="curstyle" aria-haspopup="listbox"><span class="d"><svg viewBox="0 0 24 24">${t.dot}</svg></span>${esc(t.name(state.lang))}<svg class="chev" width="8" height="12" viewBox="0 0 8 12" aria-hidden="true"><path d="M1.5 4.5 4 2l2.5 2.5M1.5 7.5 4 10l2.5-2.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg></button>`
    + `<button class="step" data-d="1">${STEP_SVG('M4.5 2.5 8 6l-3.5 3.5')}</button>`;
  const [prev, next] = nav.querySelectorAll<HTMLElement>('.step');
  prev.title = L('上一种风格', 'Previous style'); next.title = L('下一种风格', 'Next style');
  prev.setAttribute('aria-label', prev.title); next.setAttribute('aria-label', next.title);
  nav.onclick = (e) => {
    const b = (e.target as HTMLElement).closest('button') as HTMLElement | null; if (!b) return;
    if (b.dataset.d) { const i = KITS.indexOf(state.style); return void useStyle(KITS[(i + Number(b.dataset.d) + KITS.length) % KITS.length]); }
    if (b.dataset.s) { closeStyles(); return void useStyle(kitById(b.dataset.s)!); }
    if (b.id === 'curstyle') toggleStyles();
  };
}
function toggleStyles() {
  const nav = $('#dots'), open = nav.querySelector('.styles');
  if (open) return closeStyles();
  const box = document.createElement('div');
  box.className = 'styles glass'; box.setAttribute('role', 'listbox');
  box.innerHTML = KITS.map((t, i) => `${i === CLASSIC_COUNT ? '<i class="divider"></i>' : ''}<button data-s="${t.id}" role="option" aria-selected="${t === state.style}" class="${t === state.style ? 'on' : ''}"><svg viewBox="0 0 24 24">${t.dot}</svg>${esc(t.name(state.lang))}</button>`).join('');
  nav.appendChild(box);
  $('#curstyle').setAttribute('aria-expanded', 'true');
  box.querySelector<HTMLElement>('.on')?.scrollIntoView({ block: 'nearest' });
  setTimeout(() => document.addEventListener('pointerdown', outsideStyles, true));
}
function outsideStyles(e: Event) { if (!(e.target as HTMLElement).closest('#dots')) closeStyles(); }
function closeStyles() {
  $('#dots').querySelector('.styles')?.remove();
  document.querySelector('#curstyle')?.setAttribute('aria-expanded', 'false');
  document.removeEventListener('pointerdown', outsideStyles, true);
}
async function useStyle(t: Kit) {
  if (t.id === state.style.id) return;
  state.style = t; store.set('theme', t.id);
  state.variants = {};
  renderDots();
  await paint();
}

// ───────────────────────── 顶栏：项目、语言、设置 ─────────────────────────
/** 页面里写死的字、无障碍标签、<html lang> */
function applyLang() {
  document.documentElement.lang = state.lang === 'zh' ? 'zh-CN' : 'en';
  const lang = $('#lang'), gear = $('#gear');
  lang.textContent = state.lang === 'zh' ? '中' : 'EN';
  lang.title = L('切换到英文', 'Switch to Chinese');
  lang.setAttribute('aria-label', lang.title);
  gear.title = L('设置', 'Settings'); gear.setAttribute('aria-label', gear.title);
  $('#project').setAttribute('aria-label', L('切换项目', 'Switch project'));
  $('#dots').setAttribute('aria-label', L('风格', 'Styles'));
  $('#dock').setAttribute('aria-label', L('导出', 'Export'));
  const [w, r] = document.querySelectorAll<HTMLElement>('#view button');
  w.title = L('墙', 'Wall'); r.title = L('漫游', 'Roam');
  w.setAttribute('aria-label', w.title); r.setAttribute('aria-label', r.title);
  $('#view').setAttribute('aria-label', L('视图', 'View'));
  $('#home').innerHTML = `<svg width="13" height="13" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="7" cy="7" r="2.2"/><path d="M7 1v2.2M7 10.8V13M1 7h2.2M10.8 7H13"/></svg>${L('回到起点', 'Back to start')}`;
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
/** 菜单里还没有 * 的项目：在后台一个个读档案，算出来就换上 */
let filling = false;
async function fillMarks(redraw: () => void) {
  if (filling) return;
  filling = true;
  try {
    for (const p of state.projects) {
      if (markOf(p.id) || !p.lastAnalyzed || p.exists === false) continue;
      const raw = await api('credits', { projectId: p.id }).catch(() => null);
      if (!raw) continue;
      try { store.set('mark.' + p.id, JSON.stringify(petalsOf(buildModel(raw, { director: false, lang: state.lang, group: 'tool' })))); redraw(); } catch { /* 读不了的档案就留着淡的 * */ }
    }
  } finally { filling = false; }
}
function closeMenu() { $('#menu').hidden = true; $('#project').setAttribute('aria-expanded', 'false'); }
function toggleMenu() {
  const menu = $('#menu'), pill = $('#project');
  if (!menu.hidden) return closeMenu();
  pill.setAttribute('aria-expanded', 'true');
  menu.hidden = false;
  menu.innerHTML = `<input id="q" placeholder="${L('搜索项目', 'Search projects')}" autocomplete="off"><div class="list" id="plist" role="listbox" aria-label="${L('项目', 'Projects')}"></div>
    <div class="foot"><button id="add">${L('添加文件夹…', 'Add folder…')}</button><button id="re">${L('重新分析', 'Re-analyze')}</button></div>
    <div class="mplugs" id="mplugs" hidden></div>`;
  const day = (t: number) => new Date(t * 1000).toLocaleDateString(state.lang === 'zh' ? 'zh-CN' : 'en-US');
  const draw = () => {
    const q = ($('#q') as HTMLInputElement).value.trim().toLowerCase();
    const hit = (s: string) => !q || s.toLowerCase().includes(q);
    const all = state.projects.length > 1 && (hit(L('全部项目', 'All projects')) || 'all'.includes(q))
      ? `<div class="item all ${state.current?.all ? 'on' : ''}" data-id="${ALL_ID}" role="option" aria-selected="${!!state.current?.all}">${markEl(markOf(ALL_ID) ?? GHOST)}<b>${L('全部项目', 'All projects')}</b><span class="r">${L(`${state.projects.length} 个`, `${state.projects.length}`)}</span></div>` : '';
    const mine = state.projects.filter((p) => hit(`${p.name} ${p.path}`)).map((p) => `
      <div class="item ${p.id === state.current?.id ? 'on' : ''} ${p.exists === false ? 'gone' : ''}" data-id="${esc(p.id)}" role="option" aria-selected="${p.id === state.current?.id}">
        ${markEl(markOf(p.id) ?? GHOST)}<b>${esc(p.name)}</b><span class="r">${p.exists === false ? L('目录不在了', 'folder missing') : p.lastAnalyzed ? day(p.lastAnalyzed) : ''}</span>
        <span class="path" title="${esc(p.path)}">${esc(short(p.path))}</span>
      </div>`).join('');
    // 本机用过 AI 工具、还没加进来的项目：点一下就分析
    const more = (found ?? []).filter((d) => hit(`${d.name} ${d.path}`)).map((d) => `
      <div class="item new" data-path="${esc(d.path)}" role="option" aria-selected="false">
        ${markEl(GHOST)}<b>${esc(d.name)}</b><span class="r">${esc(toolsOf(d))}</span>
        <span class="path" title="${esc(d.path)}">${esc(short(d.path))}</span>
      </div>`).join('');
    $('#plist').innerHTML = all + mine + (more ? `<div class="sec">${L('还没添加', 'Not added yet')}</div>${more}` : '')
      || `<div class="item"><span>${L('没有匹配的项目', 'No match')}</span></div>`;
  };
  // ↑ ↓ 选，回车打开（没选就开第一个）
  let kb = -1;
  const items = () => [...$('#plist').querySelectorAll<HTMLElement>('.item[data-id], .item[data-path]')];
  const mark = () => items().forEach((x, j) => { x.classList.toggle('kb', j === kb); if (j === kb) x.scrollIntoView({ block: 'nearest' }); });
  draw();
  if (!demo && found === null) void discover().then(() => { if (!menu.hidden) draw(); });
  void fillMarks(() => { if (!menu.hidden) draw(); });
  renderPlugs();
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
    const it = (e.target as HTMLElement).closest('.item') as HTMLElement | null; if (!it) return;
    if (it.dataset.path) { closeMenu(); return void addProjects([it.dataset.path]); }
    if (!it.dataset.id) return;
    closeMenu();
    if (it.dataset.id === ALL_ID) { if (!state.current?.all) openProject(allProject()); return; }
    const p = state.projects.find((x) => x.id === it.dataset.id);
    if (p && p !== state.current) openProject(p);
  };
  const re = $('#re') as HTMLButtonElement;
  if (!state.current || state.current.all || state.current.exists === false || analyzing) { re.disabled = true; re.title = state.current?.exists === false ? L('项目目录已经不在了', 'The project folder is gone') : ''; }
  ($('#add') as HTMLButtonElement).disabled = analyzing;
  $('#add').onclick = () => { closeMenu(); addProject(); };
  re.onclick = () => { closeMenu(); if (state.current) reanalyze(state.current); };
  setTimeout(() => document.addEventListener('click', function off(e) { if (!menu.contains(e.target as Node) && !pill.contains(e.target as Node)) { closeMenu(); document.removeEventListener('click', off); } }), 0);
}

// ───────────────────────── 本机的项目 ─────────────────────────
// 核心的 discover 扫各工具的记录，列出用过 AI 工具的仓库。第一次打开和「添加」时用，一次会话扫一遍
type Found = { name: string; path: string; tools: Record<string, number>; lastActive: string; exists: boolean; inLibrary: boolean };
let found: Found[] | null = null, finding: Promise<Found[]> | null = null;
function discover(): Promise<Found[]> {
  finding ??= api('discover').then((r: any) => (found = ((r?.projects ?? []) as Found[]).filter((d) => d.exists && !d.inLibrary && !state.projects.some((p) => p.path === d.path))), () => (found = []));
  return finding;
}
const toolsOf = (d: Found) => Object.entries(d.tools).sort((a, b) => b[1] - a[1]).slice(0, 2).map(([t]) => nameOf(t, state.lang)).join(' · ');

// ───────────────────────── 分析：进度、取消 ─────────────────────────
let analyzing = false;
/** 跑一次分析：一直挂着进度提示（秒数、取消），跑完或出错再换成结果 */
async function withProgress<T>(name: string, job: () => Promise<T>, auto = false): Promise<T> {
  if (analyzing) throw { code: 'busy' };
  analyzing = true;
  const t0 = Date.now();
  // 核心报来的进度：大仓库第一次分析要很久，让人看到走到哪一步了
  let stage = '';
  const STAGES: Record<string, [string, string]> = { history: ['读提交历史', 'Reading history'], logs: ['读各工具的日志', 'Reading tool logs'], attribute: ['逐个提交归因', 'Attributing commits'], blame: ['追溯现在的每个文件', 'Tracing current files'] };
  const unlisten: (() => void) | null = await (window as any).__TAURI__?.event?.listen('analyze-progress', (e: any) => {
    const p = e.payload ?? {}, n = STAGES[p.stage];
    if (n) stage = `${L(...n)}${p.total ? ` ${fmt(p.done)}/${fmt(p.total)}` : ''}`;
  }).catch(() => null) ?? null;
  const text = () => { const s = Math.round((Date.now() - t0) / 1000); const st = stage ? ` · ${stage}` : ''; return auto ? L(`有新提交，正在跟上…${s ? ` ${s} 秒` : ''}${st}`, `New commits — catching up…${s ? ` ${s}s` : ''}${st}`) : L(`正在分析 ${name}…${s ? ` ${s} 秒` : ''}${st}`, `Analyzing ${name}…${s ? ` ${s}s` : ''}${st}`); };
  toast(text(), 0, { spin: true, acts: [{ label: L('取消', 'Cancel'), run: () => { toast(L('正在取消…', 'Cancelling…'), 0, { spin: true }); api('cancel_analyze').catch(() => null); } }] });
  const tick = setInterval(() => { const m = document.querySelector('#toast .msg'); if (m && document.querySelector('#toast button')) m.textContent = text(); }, 1000);
  try { return await job(); } finally { clearInterval(tick); unlisten?.(); analyzing = false; }
}
async function reanalyze(p: any, auto = false) {
  try {
    const v: any = await withProgress(p.name, () => api('analyze', { projectId: p.id, lang: state.lang }), auto);
    state.projects = await api('projects');
    const fresh = state.projects.find((x) => x.id === p.id) ?? p;
    // 正看着这个项目：原地换，不清空墙
    if (state.current?.id === p.id && state.raw) {
      const st = readStamp(fresh);
      const raw = await api('credits', { projectId: p.id });
      state.current = fresh; stampOf = await st;
      await swapRaw(raw);
    } else await openProject(fresh);
    const n = Number(v?.newCommits) || 0;
    const mk = markOf(p.id);
    if (auto) toast(n ? L(`已跟上 ${n} 个新提交`, `Caught up with ${n} new commit${n > 1 ? 's' : ''}`) : L('已更新', 'Updated'), 2600, { mark: mk });
    else done(L('已更新', 'Updated'), v?.warnings, mk);
  } catch (e) { if (!auto || (e as any)?.code !== 'busy') toast(errText(e), 6000); }
}
async function addProject() {
  let dir: string | null;
  try { dir = await api('pick_folder'); } catch (e) { return toast(errText(e), 5000); }
  if (dir) await addProjects([dir]);
}
/** 一个接一个分析、加进作品库。第一个分析完就打开它，其余的在后台接着跑（提示条里看进度、能取消） */
async function addProjects(paths: string[], each?: (path: string, st: 'run' | 'done' | 'fail') => void) {
  let opened = false, ok = 0;
  const notes: string[] = [];
  for (const [i, path] of paths.entries()) {
    const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
    each?.(path, 'run');
    try {
      const r = await withProgress(paths.length > 1 ? L(`${name}（${i + 1}/${paths.length}）`, `${name} (${i + 1}/${paths.length})`) : name, () => api('add_project', { path, lang: state.lang }));
      state.projects = await api('projects'); ok++;
      if (found) found = found.filter((d) => d.path !== path);
      notes.push(...(r?.result?.warnings ?? []));
      each?.(path, 'done');
      const p = state.projects.find((x) => x.id === r?.id);
      if (p && !opened) { opened = true; $('#welcome').hidden = true; void openProject(p); }
    } catch (e: any) {
      each?.(path, 'fail');
      if (e?.code === 'cancelled') { toast(errText(e)); return; }
      toast(L(`${name}：${errText(e)}`, `${name}: ${errText(e)}`), 6000);
    }
  }
  if (ok) done(ok > 1 ? L(`已添加 ${ok} 个项目`, `Added ${ok} projects`) : L('已添加', 'Added'), notes);
}

// ───────────────────────── 第一次打开 ─────────────────────────
// 作品库是空的：列出本机用过 AI 工具的仓库，勾几个就开始分析；没找到就选文件夹
function welcome() {
  const el = $('#welcome');
  el.hidden = false; $('#empty').hidden = true; $('#board').innerHTML = '';
  applyView();
  const picked = new Set<string>();
  let list: Found[] | null = null;
  const status = new Map<string, string>();
  let first = true; // 标志只在第一次画出来时长一遍
  const draw = () => {
    if (el.hidden) return;
    const running = [...status.values()].includes('run');
    el.innerHTML = `<div class="box">
      <span class="logo${first && logoNow().startsWith('star') ? ' bloom' : ''}">${logoSvg(store.get('logo'))}</span>
      <h1>${L('选择要分析的项目', 'Select projects to start')}</h1>
      <div class="found">${list === null ? `<p class="wait">${SPIN()}${L('正在扫描本地包含 AI 工具记录的 Git 仓库…', 'Looking for repos where you used AI tools…')}</p>`
        : list.length ? list.map((d) => `<label class="row ${status.get(d.path) ?? ''}" data-path="${esc(d.path)}">
            <input type="checkbox" ${picked.has(d.path) ? 'checked' : ''} ${running || status.has(d.path) ? 'disabled' : ''}>
            <b>${esc(d.name)}</b><span class="path" title="${esc(d.path)}">${esc(short(d.path))}</span><span class="tools">${esc(toolsOf(d))}</span>
            <i class="st">${status.get(d.path) === 'run' ? SPIN() : ''}</i></label>`).join('')
        : `<p class="wait">${L('未发现包含 AI 工具记录的 Git 仓库', 'No git repos with AI tool activity found')}</p>`}</div>
      <div class="acts"><button class="pick" ${running ? 'disabled' : ''}>${L('选择文件夹…', 'Choose folder…')}</button>${list?.length ? `<button class="go" ${!picked.size || running ? 'disabled' : ''}>${running ? L('分析中…', 'Analyzing…') : picked.size > 1 ? L(`分析这 ${picked.size} 个`, `Analyze ${picked.size}`) : L('开始分析', 'Analyze')}</button>` : ''}</div>
    </div>`;
    first = false;
  };
  el.onchange = (e) => {
    const box = e.target as HTMLInputElement, path = box.closest<HTMLElement>('.row')?.dataset.path;
    if (!path) return;
    if (box.checked) picked.add(path); else picked.delete(path);
    draw();
  };
  el.onclick = async (e) => {
    const b = (e.target as HTMLElement).closest('button'); if (!b || b.disabled) return;
    if (b.classList.contains('pick')) return addProject();
    const paths = list!.map((d) => d.path).filter((p) => picked.has(p));
    await addProjects(paths, (path, st) => { status.set(path, st); draw(); });
  };
  draw();
  void discover().then((r) => { list = r; r.slice(0, 3).forEach((d) => picked.add(d.path)); draw(); });
}

/** 分析跑完：核心有提醒（某个数据源读不了之类）就在提示里给个「查看」 */
function done(msg: string, warnings?: string[], mark?: Petal[] | null) {
  if (!warnings?.length) return toast(msg, 2600, { mark });
  toast(L(`${msg} · ${warnings.length} 条提醒`, `${msg} · ${warnings.length} ${warnings.length > 1 ? 'notes' : 'note'}`), 10000, { mark, acts: [{ label: L('查看', 'View'), run: () => showWarnings(warnings) }] });
}
function showWarnings(list: string[]) {
  const el = $('#sheet');
  el.hidden = false;
  el.innerHTML = `<div class="panel glass">
    <div class="head"><h2>${L('分析时遇到的问题', 'Issues during analysis')}</h2>${closeBtn()}</div>
    <ul class="warn">${list.map((w) => `<li>${esc(w)}</li>`).join('')}</ul>
  </div>`;
  el.onclick = (e) => { if (e.target === el || (e.target as HTMLElement).closest('[data-act="close"]')) closeSheet(); };
}

function closeSheet() { const el = $('#sheet'); el.hidden = true; el.innerHTML = ''; }
const logoNow = () => (store.get('logo') && LOGOS[store.get('logo')!] ? store.get('logo')! : DEFAULT_LOGO);
async function settings() {
  const el = $('#sheet');
  el.hidden = false;
  const info = native ? await api('app_info').catch(() => null) : null;
  const coll = await api('collect_info').catch(() => null);
  el.innerHTML = `<div class="panel glass">
    <div class="head"><h2>${L('设置', 'Settings')}</h2>${closeBtn()}</div>
    <div class="row kv"><span class="k">${L('标志', 'Logo')}</span><span class="v grow"></span><div class="logos" role="radiogroup" aria-label="${L('标志', 'Logo')}">${Object.entries(LOGOS).map(([id, g]) => `<button data-x="logo" data-id="${id}" role="radio" aria-checked="${id === logoNow()}" title="${esc(L(...g.name))}" aria-label="${esc(L(...g.name))}">${g.svg}</button>`).join('')}</div></div>
    <h3>${L('导出', 'Export')}</h3>
    ${info ? `<div class="row kv" id="xrow"></div>` : ''}
    <div class="row kv"><span class="k">${L('默认尺寸', 'Default size')}</span><div class="seg" id="defsize">${sizeButtons()}</div></div>
    <h3>${L('本地项目库', 'Library')}</h3>
    ${info ? `<div class="row kv"><span class="k">${L('位置', 'Location')}</span><span class="v path" title="${esc(info.library)}">${esc(short(info.library))}</span><button class="mini" data-x="open-library">${L('打开', 'Open')}</button></div>` : ''}
    <div class="row kv"><span class="k">${L('有新提交时自动更新', 'Update on new commits')}</span><span class="v grow"></span><button class="switch" role="switch" aria-checked="${autoFresh()}" aria-label="${L('有新提交时自动更新', 'Update on new commits')}" data-x="fresh"><i></i></button></div>
    ${coll ? `<div class="row kv" id="crow"></div>` : ''}
    <h3>${L('AI 工具', 'AI tools')}</h3><div id="doc"><p>${L('检查中…', 'Checking…')}</p></div>
    <p class="about">${info ? `creditia ${esc(info.version)} · ` : ''}${L('只在本机运行', 'Runs only on this computer')}</p>
  </div>`;
  const xrow = (x: any) => {
    const r = el.querySelector('#xrow'); if (!r) return;
    r.innerHTML = `<span class="k">${L('保存到', 'Save to')}</span><span class="v path" title="${esc(x.exportDir)}">${esc(short(x.exportDir))}</span><button class="mini" data-x="choose">${L('更改…', 'Change…')}</button>${x.exportDefault ? '' : `<button class="mini" data-x="reset">${L('恢复默认', 'Reset')}</button>`}<button class="mini" data-x="open-exports">${L('打开', 'Open')}</button>`;
  };
  if (info) xrow(info);
  const ago = (t: number) => {
    const m = Math.floor((Date.now() / 1000 - t) / 60), h = Math.floor(m / 60), d = Math.floor(h / 24);
    return m < 1 ? L('刚刚', 'just now') : m < 60 ? L(`${m} 分钟前`, `${m} min ago`) : h < 24 ? L(`${h} 小时前`, `${h} h ago`) : L(`${d} 天前`, `${d} d ago`);
  };
  const crow = (c: any) => {
    const r = el.querySelector('#crow'); if (!r || !c) return;
    // 定时把新记录同步至本地项目库。三档：不备份 / App 开着时 / 一直（macOS 的后台任务）
    const mode = c.agent?.on ? 'always' : c.on ? 'open' : 'off';
    const modes: [string, string][] = [['off', L('关', 'Off')], ['open', L('App 开着时', 'While open')], ...(c.agent?.supported ? [['always', L('一直', 'Always')]] as [string, string][] : [])];
    const st = c.running ? L('正在备份…', 'Backing up…') : mode !== 'off' && c.last?.at ? L(`上次 ${ago(c.last.at)}`, `Last ${ago(c.last.at)}`) : '';
    r.innerHTML = `<span class="k" title="${L('将本地编程工具记录同步至本地存档，防止历史记录遗失', 'Sync local tool logs to the local project archive')}">${L('备份 AI 工具记录', 'Back up AI tool logs')}</span><span class="v grow">${esc(st)}</span>`
      + `<div class="seg" role="radiogroup">${modes.map(([m, n]) => `<button data-x="collect-mode" data-m="${m}" role="radio" aria-checked="${m === mode}" class="${m === mode ? 'on' : ''}">${esc(n)}</button>`).join('')}</div>`;
    // 一轮正在跑（定时的那轮）：跑完换成结果
    if (c.running) setTimeout(() => { if (el.querySelector('#crow')) api('collect_info').then(crow).catch(() => null); }, 2000);
  };
  crow(coll);
  el.onclick = async (e) => {
    const t = e.target as HTMLElement;
    if (e.target === el || t.closest('[data-act="close"]')) return closeSheet();
    const sz = t.closest('#defsize button') as HTMLElement | null;
    if (sz) return setSize(sz.dataset.s!);
    const x = (t.closest('[data-x]') as HTMLElement | null)?.dataset.x;
    try {
      if (x === 'choose') { if (await api('choose_export_dir')) xrow(await api('app_info')); }
      else if (x === 'reset') { await api('reset_export_dir'); xrow(await api('app_info')); }
      else if (x === 'open-exports') await api('open_folder', { which: 'exports' });
      else if (x === 'open-library') await api('open_folder', { which: 'library' });
      else if (x === 'fresh') {
        store.set('autofresh', autoFresh() ? 'off' : 'on');
        el.querySelector('[data-x="fresh"]')!.setAttribute('aria-checked', String(autoFresh()));
        if (autoFresh()) refreshIfChanged();
      }
      else if (x === 'logo') {
        const id = (t.closest('[data-id]') as HTMLElement).dataset.id!;
        await api('set_logo', { id });
        store.set('logo', id);
        el.querySelectorAll('.logos button').forEach((b) => b.setAttribute('aria-checked', String((b as HTMLElement).dataset.id === id)));
      }
      else if (x === 'collect-mode') {
        const m = (t.closest('[data-m]') as HTMLElement).dataset.m!;
        let c = await api('set_collect', { on: m !== 'off' });
        if (c.agent?.supported && !!c.agent.on !== (m === 'always')) c = await api('set_collect_agent', { on: m === 'always' });
        crow(c);
      }
    } catch (err) { toast(errText(err), 5000); }
  };
  /** doctor 的数据源名：一个工具有两处记录时分开写 */
  const SOURCES: Record<string, [string, string]> = { 'copilot-vscode': ['Copilot（VS Code）', 'Copilot (VS Code)'], 'copilot-cli': ['Copilot 命令行', 'Copilot CLI'], 'cursor-cli': ['Cursor 命令行', 'Cursor CLI'], 'cline-cli': ['Cline 命令行', 'Cline CLI'], 'amazon-q-ide': ['Amazon Q（编辑器）', 'Amazon Q (IDE)'], 'costrict-ide': ['CoStrict（编辑器）', 'CoStrict (IDE)'], 'trae-cli': ['TRAE 命令行', 'TRAE CLI'], 'trae-ide': ['TRAE IDE', 'TRAE IDE'], 'kilo-cli': ['Kilo 命令行', 'Kilo CLI'], antigravity: ['Antigravity 命令行', 'Antigravity CLI'], windsurf: ['Windsurf Cascade', 'Windsurf Cascade'] };
  Object.assign(SOURCES, { 'kiro-cli-classic': ['Kiro CLI（classic）', 'Kiro CLI (classic)'], 'tabnine-ide': ['Tabnine（编辑器）', 'Tabnine (IDE)'], 'devin-cloud': ['Devin 云端', 'Devin Cloud'], 'devin-cli': ['Devin CLI', 'Devin CLI'] });
  Object.assign(SOURCES, { 'warp-oz': ['Warp Oz 运行信息', 'Warp Oz run information'] });
  Object.assign(SOURCES, { 'kiro-kas-v3': ['Kiro CLI（V3 会话）', 'Kiro CLI (V3 sessions)'], 'kiro-cli-v2': ['Kiro CLI（V2 会话）', 'Kiro CLI (V2 sessions)'] });
  Object.assign(SOURCES, { 'trae-native': ['TRAE（编辑器历史）', 'TRAE (editor history)'] });
  const sourceName = (id: string) => id.endsWith('-acp') ? `${nameOf(id.slice(0, -4), state.lang)} ${L('采集', 'capture')}` : (SOURCES[id] ? L(...SOURCES[id]) : nameOf(id.replace(/-cli$/, ''), state.lang));
  const doc = await api('doctor').catch(() => []);
  const detail = (d: any) => d.scope === 'sessions' && d.ok ? L('仅会话信息', 'Session information only')
    : d.scope === 'capture' && !d.ok ? L('需开启采集', 'Capture required')
    : d.unsupported ? L('编辑格式待接', 'Edit format unconfirmed')
    : !d.ok ? L('没有找到', 'Not found')
    : d.records != null ? L(`${fmt(d.records)} 条记录`, `${fmt(d.records)} records`)
    : d.entries != null ? L(`${fmt(d.entries)} 个条目`, `${fmt(d.entries)} entries`)
    : L('已就绪', 'Ready');
  $('#doc').innerHTML = doc.length ? doc.map((d: any) => `<div class="row" title="${esc(d.path ?? '')}"><i class="dot ${d.ok && !d.unsupported ? 'ok' : ''}"></i><span class="k">${esc(sourceName(d.id))}</span><span class="v">${esc(detail(d))}</span></div>`).join('') : `<p>${L('没有可用的数据源', 'No data sources found')}</p>`;
}

boot();
