// creditia App：一面展墙。顶部切换项目，左下角圆点切换风格，所有作品直接摆出来；
// 悬停看细节、换版式、选格式导出；拖起作品时下面浮出导出坞，松手就导出。
import { buildModel, type Model } from '../../../packages/render/src/classic/data.ts';
import { nameOf, resolveGroup, buildModel as buildText } from '../../../packages/render/src/data.ts';
import { MODEL_VARIANTS } from '../../../packages/render/src/registry.ts';
import { mergeCredits } from '../../../packages/render/src/merge.ts';
// CREDITS.md 和命令行 creditia doc 用同一份生成代码
import { creditsBlock, mergeCreditsMd } from '../../../packages/render/src/text.ts';
import { FORMATS, SIZES, type Art } from '../../../packages/render/src/export.ts';
import { INTERACT_CSS, mountHits, placeTip } from '../../../packages/render/src/interact.ts';
import { KITS, CLASSIC_COUNT, kitById, bootKits, setGroup, setNamed, groupOf, hasModels, type Kit } from './kits.ts';
import { exportArt, type Format } from './engine.ts';
import { mountRoamPreview, releaseRoamPreview } from './roam-preview.ts';
import { RoamViewport } from './roam-viewport.ts';
import { wallTiles, tileKey as keyOf, wallVariant, selectWallVariant } from './wall.ts';
import { LOGOS, DEFAULT_LOGO, logoSvg } from './logo.ts';
import { markPaths, sharePetals, type Petal } from '../../../packages/render/src/mark.ts';
import { icon } from './icons.ts';
import { LiveMark, fly, BRAND, GHOST as UNREAD } from './live-mark.ts';

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
/** 浏览器里移除、关联的项目和路径只记在内存里 */
const devLib = { removed: new Set<string>(), linked: new Map<string, string[]>() };
async function api(cmd: string, args: any = {}): Promise<any> {
  if (native) return native(cmd, args);
  const get = (u: string) => fetch(u).then((r) => { if (!r.ok) throw new Error(r.statusText); return r.json(); });
  switch (cmd) {
    case 'projects': {
      const list: any[] = demo ? demo.projects : devFresh.on && !devFresh.added ? [] : await get('/__dev/projects');
      return list.filter((p) => !devLib.removed.has(p.id)).map((p) => ({ ...p, paths: [...(p.paths ?? [p.path]), ...(devLib.linked.get(p.id) ?? [])], pathKinds: { ...(p.pathKinds ?? {}), ...Object.fromEntries((devLib.linked.get(p.id) ?? []).map((x) => [x, 'manual'])) } }));
    }
    case 'remove_project': devLib.removed.add(args.projectId); return null;
    case 'link_path': {
      const l = devLib.linked.get(args.projectId) ?? [];
      devLib.linked.set(args.projectId, args.unlink ? l.filter((x) => x !== args.path) : [...l, args.path]);
      return { summary: args.unlink ? L(`已取消关联 ${args.path}`, `Unlinked ${args.path}`) : L(`已关联 ${args.path}`, `Linked ${args.path}`) };
    }
    case 'pick_folder': if (demo) throw new Error(L('这一步要在 App 里运行', 'Only available in the desktop app')); return '/Users/me/code/catnap-wt';
    case 'discover': if (demo) throw new Error('demo'); await new Promise((r) => setTimeout(r, 900)); return { projects: [
      { name: 'catnap', path: '/Users/me/code/catnap', tools: { 'claude-code': 42, codex: 12 }, lastActive: '2026-10-20', exists: true, inLibrary: !devFresh.on },
      { name: 'atlas', path: '/Users/me/code/atlas', tools: { cursor: 30 }, lastActive: '2026-10-12', exists: true, inLibrary: false },
      { name: 'dotfiles', path: '/Users/me/dotfiles', tools: { 'claude-code': 4 }, lastActive: '2026-09-02', exists: true, inLibrary: false },
      { name: 'scratch', path: '/Users/me/tmp/scratch', tools: { codex: 3 }, lastActive: '2026-08-30', exists: true, inLibrary: false },
    ] };
    case 'add_project': if (demo) throw new Error('demo'); await devAnalyze(); devFresh.added = true; return { id: (await get('/__dev/projects'))[0]?.id, result: {} };
    case 'analyze': if (demo) throw new Error(L('这一步要在 App 里运行', 'Only available in the desktop app')); await devAnalyze(); return { newCommits: 0 };
    case 'cancel_analyze': devCancel = true; return null;
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
    case 'open_folder': return null;
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
/** 快捷键提示里的修饰键 */
const MOD = OS === 'mac' ? '⌘' : 'Ctrl+';
const revealLabel = () => (OS === 'mac' ? L('在访达中显示', 'Show in Finder') : OS === 'win' ? L('在资源管理器中显示', 'Show in Explorer') : L('打开所在文件夹', 'Open folder'));

const tiles = () => wallTiles(state.style);

// ───────────────────────── 星号 *：App 的状态都由它来表达 ─────────────────────────
// 一颗 * 两个位置：平时在左上角项目名前面，是这个项目自己的份额；屏幕上没有别的可看时（读第一屏、挡在前面的分析）
// 放到中间。出了结果，先在中间停一会儿让人看清，再飞回左上角——看过这一下，之后它在角落动，人才知道往那儿看。
// 等待按时长分档：300ms 内不打扰；到 2 秒只让 * 动；再久加一句话（中间、第一次打开页，或者项目名后面展开的一段）。
const mark = new LiveMark({ optical: true });
/** 正在跑的分析。blocking：用户在等它（没有墙可看）；auto：后台跟上新提交 */
type Job = { name: string; auto: boolean; blocking: boolean; t0: number; stage: string; p: number | null; cancelling?: boolean };
let job: Job | null = null;
/** 正在读一个项目的档案、画第一屏 */
let loading: { name: string; t0: number } | null = null;
let markErr = false, flashText = '', flashTimer = 0, errTimer = 0, centerHold = 0, statusTick = 0;
/** 第一次打开页在扫描本机的仓库 */
let scanning = false;
const painted = () => !!document.querySelector('#board .tile:not(.slot), #roam .rt');
/** * 现在该在哪：第一次打开页、空状态、中间，否则左上角 */
function markHome(): HTMLElement {
  if (!$('#welcome').hidden) { const w = document.querySelector<HTMLElement>('#welcome .logo'); if (w) return w; }
  if (!$('#empty').hidden) { const e = document.querySelector<HTMLElement>('#empty .mslot'); if (e) return e; }
  if (!$('#center').hidden) return $('#center .mslot');
  return $('#pmark');
}
function settleMark() { const to = markHome(); if (mark.el.parentElement !== to) fly(mark.el, to); }
function jobText(j: Job) {
  const s = Math.round((Date.now() - j.t0) / 1000), secs = s ? L(` · ${s} 秒`, ` · ${s}s`) : '';
  if (j.cancelling) return L('正在取消…', 'Cancelling…');
  if (j.auto) return L(`有新提交，正在跟上…${secs}`, `New commits — catching up…${secs}`);
  // 左上角已经写着这个项目的名字，就不再重复
  const who = $('#center').hidden && $('#welcome').hidden && j.name === state.current?.name ? '' : L(`分析 ${j.name}`, `Analyzing ${j.name}`);
  return [who, j.stage || (who ? '' : L('正在分析', 'Analyzing'))].filter(Boolean).join(' · ') + secs;
}
let lastBar = '';
/** 程序坞（任务栏）上的进度，和 * 同步 */
function dockBar() {
  const win = (window as any).__TAURI__?.window?.getCurrentWindow?.();
  if (!win) return;
  const v = job && !job.auto ? (job.p == null ? { status: 'indeterminate' } : { status: 'normal', progress: Math.round(job.p * 100) }) : { status: 'none' };
  const key = JSON.stringify(v);
  if (key !== lastBar) { lastBar = key; void win.setProgressBar(v).catch(() => null); }
}
function syncStatus() {
  const since = job ? Date.now() - job.t0 : loading ? Date.now() - loading.t0 : 0;
  if (scanning && $('#welcome').hidden) scanning = false;
  mark.setMode(markErr ? 'error' : job && !job.auto ? 'progress' : job || loading || scanning ? 'loading' : 'idle');
  if (job && !job.auto) mark.progress(job.p);
  // 挡在前面、而且没有墙可看：到中间
  const wantCenter = $('#welcome').hidden && $('#empty').hidden && !painted() && (!!job?.blocking || (!!loading && since >= 300));
  if (wantCenter) { clearTimeout(centerHold); centerHold = 0; $('#center').hidden = false; }
  else if (!$('#center').hidden && !centerHold) centerHold = window.setTimeout(() => { centerHold = 0; $('#center').hidden = true; settleMark(); syncStatus(); }, 650);
  settleMark();
  // 一句话：超过 2 秒才说；刚做完的结果（flash）立刻说
  const text = job ? jobText(job) : loading ? L(`正在读取 ${loading.name}…`, `Opening ${loading.name}…`) : flashText;
  const show = (job || loading) ? since >= 2000 : !!flashText;
  const cancel = !!job && !job.auto && !job.cancelling;
  const html = show && text ? `<span class="txt">${esc(text)}</span>${cancel ? `<button data-act="cancel">${L('取消', 'Cancel')}</button>` : ''}` : '';
  const center = !$('#center').hidden, wel = !$('#welcome').hidden;
  const put = (el: HTMLElement | null, on: boolean) => { if (!el) return; const h = on ? html : ''; if (el.innerHTML !== h) el.innerHTML = h; el.classList.toggle('on', on && !!html); };
  put($('#center .lbl'), center);
  put(document.querySelector<HTMLElement>('#welcome .wstat'), wel && !center);
  put($('#pstat .in'), !center && !wel);
  $('#pstat').classList.toggle('on', !center && !wel && !!html);
  dockBar();
  // 等待中每半秒刷一次（秒数、分档）；都结束了就停
  if ((job || loading) && !statusTick) statusTick = window.setInterval(syncStatus, 500);
  if (!job && !loading && statusTick) { clearInterval(statusTick); statusTick = 0; }
}
/** 刚做完的一句结果，写在项目名后面，过一会儿收起 */
function flash(text: string, ms = 2400) {
  flashText = text; clearTimeout(flashTimer);
  flashTimer = window.setTimeout(() => { flashText = ''; syncStatus(); }, ms);
  syncStatus();
}
/** 出错：* 收短、变灰、抖两下，过一会儿恢复；文字交给提示条 */
function markError(ms = 2400) {
  markErr = true; clearTimeout(errTimer); syncStatus();
  if (ms) errTimer = window.setTimeout(() => { markErr = false; syncStatus(); }, ms);
}
/** 第一屏有作品了：读取结束，* 回左上角 */
function loaded() { if (loading && painted()) { loading = null; syncStatus(); } }
/** 悬停左上角的 *：各工具的份额 */
function wireMarkTip() {
  const host = $('#pmark');
  const tip = () => document.querySelector<HTMLElement>('.ct-tip') ?? document.body.appendChild(Object.assign(document.createElement('div'), { className: 'ct-tip' }));
  host.addEventListener('mouseenter', (e) => {
    const m = state.model; if (!m || job || loading) return;
    const rows = m.rows.filter((r) => r.kind === 'agent' || r.kind === 'unknown' || r.kind === 'other');
    const t = tip();
    t.innerHTML = `<b>${esc(m.project || state.current?.name || '')}</b>${rows.map((r) => `<span>${esc(r.name)} · ${r.pct.toFixed(1)}%</span>`).join('')}`;
    placeTip(t, e.clientX, e.clientY); t.classList.add('on');
  });
  host.addEventListener('mousemove', (e) => { const t = document.querySelector<HTMLElement>('.ct-tip'); if (t) placeTip(t, e.clientX, e.clientY); });
  host.addEventListener('mouseleave', () => document.querySelector('.ct-tip')?.classList.remove('on'));
  // 取消按钮可能在项目名后面、中间或者第一次打开页里
  document.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest('[data-act="cancel"]'); if (!b || !job) return;
    e.stopPropagation();
    job.cancelling = true; syncStatus();
    api('cancel_analyze').catch(() => null);
  }, true);
}

// ───────────────────────── 启动 ─────────────────────────
async function boot() {
  bootKits(); state.style.warm();
  const st = document.createElement('style'); st.textContent = INTERACT_CSS; document.head.appendChild(st);
  if (native && OS === 'mac') document.body.classList.add('mac');
  if (native) void api('logo').then((id) => store.set('logo', String(id))).catch(() => null);
  document.querySelectorAll<HTMLElement>('[data-icon]').forEach((el) => (el.innerHTML = icon(el.dataset.icon!, el.closest('.pill') ? 14 : 16)));
  $('#pmark').appendChild(mark.el); mark.set(BRAND, { grow: true });
  applyLang(); renderDots(); renderDock(); wireTop(); wireRoam(); wireMarkTip(); wireKeys(); void installMenu();
  // 滚动之后顶栏垫一层渐隐的玻璃，作品不再直接透到项目名和按钮后面
  const stageEl = $('#stage');
  stageEl.addEventListener('scroll', () => document.body.classList.toggle('scrolled', stageEl.scrollTop > 8), { passive: true });
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
  try { state.projects = await api('projects'); } catch (e: any) { return empty(() => [L('无法读取本地项目记录', 'Cannot read local projects'), errText(e)], false, true); }
  if (!state.projects.length) return welcome();
  const last = (store.get('project') === ALL_ID && state.projects.length > 1 ? allProject() : null) ?? state.projects.find((p) => p.id === store.get('project')) ?? state.projects[0];
  await openProject(last);
}
/** 空状态。传进来的是函数：换语言时按新语言重写一遍 */
type EmptyView = () => [title: string, detail?: string, cmd?: string, act?: Act];
let emptyView: EmptyView | null = null, emptyErr = false;
/** error：读不到、出了错（* 变灰）；否则是还没分析过（* 是淡淡的短瓣） */
function empty(view: EmptyView, keepName = false, error = false) {
  emptyView = view; emptyErr = error;
  const [title, detail = '', cmd = '', act] = view();
  $('#board').innerHTML = ''; document.body.classList.remove('busy');
  const el = $('#empty'); el.hidden = false;
  applyView();
  el.innerHTML = `<div><span class="mslot"></span><b>${esc(title)}</b>${esc(detail)}${act ? `<br><button class="go">${esc(act.label)}</button>` : cmd ? `<br><code>${esc(cmd)}</code>` : ''}</div>`;
  if (act) (el.querySelector('.go') as HTMLElement).onclick = act.run;
  if (!keepName) $('#pname').textContent = 'creditia';
  loading = null;
  if (error) markError(0); else { markErr = false; mark.set(UNREAD); }
  syncStatus();
}

async function openProject(p: any) {
  closeFocus?.();
  const seq = ++state.openSeq;
  const prev = state.current;
  state.current = p; store.set('project', p.id);
  state.raw = null; state.model = null; state.arts.clear(); state.variants = {};
  roamReset(); // 读新档案前释放旧漫游，旧项目排队的预览不再占用绘图队列
  $('#group').hidden = true;
  ++state.token; // 让正在画的旧项目停下
  $('#empty').hidden = true;
  $('#board').innerHTML = ''; slotWall(); // 档案还在读，先把墙的样子摆出来
  $('#pname').textContent = p.name;
  // * 先换成这个项目上次记下的样子（换项目时转一瓣），读档案期间呼吸；读太久、墙上又什么都没有，就到中间
  markErr = false;
  mark.set(markOf(p.id) ?? UNREAD, { turn: prev && prev.id !== p.id ? -1 : 0 });
  loading = { name: p.name, t0: Date.now() };
  syncStatus();
  setTimeout(syncStatus, 320);
  let raw: any;
  const st = readStamp(p); // 先记下版本再读档案：读的过程中又变了，下次比对还能发现
  try { raw = await creditsOf(p); }
  catch (e: any) {
    if (seq !== state.openSeq) return;
    if (e?.code && e.code !== 'no_credits') return empty(() => [L('读不到这个项目', 'Cannot read this project'), errText(e)], true, true);
    const can = native && p.exists !== false;
    return empty(() => [L('这个项目还没分析过', 'Not analyzed yet'), '', `cd ${p.path} && creditia analyze`, can ? { label: L('现在分析', 'Analyze now'), run: () => reanalyze(p) } : undefined], true);
  }
  if (seq !== state.openSeq) return; // 已经切到别的项目
  const stamp = await st;
  if (seq !== state.openSeq) return;
  stampOf = stamp;
  state.raw = raw;
  $('#empty').hidden = true; emptyView = null;
  setGroup(groupPref()); setNamed(namesOn());
  rebuildModel();
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
    flash(L('已更新到最新提交', 'Updated to the latest commits'));
  } catch { /* 读不到就等下一次 */ } finally { refreshing = false; }
}
/** 跟上新提交：仓库里有了档案之后的新提交，就在后台增量分析一次（只处理新的提交，通常几秒）。
 *  同一个提交只试一次，失败了不反复跑；项目菜单里能关 */
/** 全局默认在设置里；每个项目可以单独开、关，没单独设就跟着默认 */
const autoDefault = () => store.get('autofresh') !== 'off';
const autoOwn = (id: string) => { const v = store.get('autofresh.' + id); return v === 'on' || v === 'off' ? v : null; };
const autoFresh = (p: any) => { const v = p?.id ? autoOwn(p.id) : null; return v ? v === 'on' : autoDefault(); };
let triedHead = '';
async function keepUp(p: any) {
  if (!autoFresh(p) || p.all || p.exists === false || !state.raw?.head) return;
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
  state.model = buildModel(state.raw, { director: false, lang: state.lang, group: groupPref(), named: namesOn() });
  const m = state.model;
  $('#pname').textContent = m.project || state.current?.name;
  const petals = petalsOf(m);
  if (state.current?.id) store.set('mark.' + state.current.id, JSON.stringify(petals));
  mark.set(petals);
  renderGroup();
  renderPlugs();
}

// ───────────────────────── 接入：提交盖章、跟着发版 ─────────────────────────
// 两项都会改用户的仓库，只在项目设置里摆两个开关，不在墙上问。开关交给核心的 hook / release install|uninstall
type Plug = { installed: boolean; decided: boolean | null; path: string; github?: boolean; published?: boolean };
type Plugs = { hook: Plug; release: Plug };
let plugs: Plugs | null = null, plugBusy = '', plugProject: any = null;
async function loadPlugs(p: any) {
  try { const r = await api('integrations', { projectId: p.id }); if (p === plugProject) plugs = r; }
  catch { if (p === plugProject) plugs = null; }
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
  document.querySelector('#plugwait')?.remove();
  m.innerHTML = plugRow('hook') + plugRow('release');
  m.querySelectorAll<HTMLElement>('button[data-k]').forEach((b) => (b.onclick = (e) => { e.stopPropagation(); const k = b.dataset.k as 'hook' | 'release'; setPlug(k, !plugs![k].installed); }));
}
async function setPlug(k: 'hook' | 'release', on: boolean, quiet = false) {
  const p = plugProject; if (!p || plugBusy) return;
  plugBusy = k; renderPlugs();
  try {
    const r = await api('set_integration', { projectId: p.id, which: k, on, lang: state.lang });
    if (!quiet && r?.summary) toast(String(r.summary), 7000);
  } catch (e: any) { toast(errText(e), 8000); }
  finally { plugBusy = ''; }
  await loadPlugs(p);
}

// ───────────────────────── 分组：按工具 / 按模型 ─────────────────────────
// 没点过就按工具；点了就钉住这个项目的选择。档案里一个模型都没记下就不显示。
type GroupPref = 'auto' | 'tool' | 'model';
/** 写不写其他贡献者的名字：设置里的开关，默认写 */
const namesOn = () => store.get('names') !== 'off';
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
  const key = `${kit.id}|${state.lang}|${groupPref()}|${namesOn()}|${id}|${variant ?? ''}`;
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
  if (wallStale) { board.innerHTML = ''; board.inert = false; document.body.classList.remove('busy'); return; }
  // 新风格先在独立容器里排占位。可视区完成就一次换墙，屏幕外继续填图。
  const batch = board.querySelector('.tile:not(.slot)') !== null;
  // 旧墙只作等待时的预览，不能拿新风格去操作仍显示的旧作品。
  board.inert = batch;
  document.body.classList.toggle('busy', batch);
  const list = tiles();
  const target = batch ? document.createElement('div') : board;
  const slots = batch ? list.map((b) => target.appendChild(slotFor(b, curVariant(keyOf(b))))) : slotWall();
  if (batch) layout(target);
  const arts = new Map<string, Art>();
  let published = !batch;
  if (published) state.arts = arts;
  const sc = $('#stage').scrollTop, vh = innerHeight;
  const distance = (i: number) => {
    const g = slots[i], y = parseFloat(g.style.top) || 0;
    const h = (parseFloat(g.style.width) || 0) * Number(g.dataset.h) / Number(g.dataset.w);
    return Math.max(0, y - (sc + vh), sc - y - h);
  };
  const order = list.map((_, i) => i).sort((a, b) => distance(a) - distance(b) || a - b);
  const visible = new Set(order.filter((i) => distance(i) === 0));
  if (!visible.size && order.length) visible.add(order[0]);
  const publish = () => {
    if (published || visible.size) return;
    state.arts = arts;
    board.replaceChildren(...target.children);
    board.style.width = target.style.width; board.style.height = target.style.height;
    board.inert = false;
    published = true;
    loaded();
  };
  let done = 0;
  ring(0);
  for (const i of order) {
    const b = list[i];
    const k = keyOf(b);
    let variant: string | undefined;
    let a: Art | null = null, preview: Blob | null = null;
    try {
      do {
        variant = curVariant(k);
        a = await drawArt(b.id, variant);
        preview = a && token === state.token ? await st.preview(a) : null;
      } while (token === state.token && variant !== curVariant(k));
    } catch (e) {
      if (token !== state.token) return;
      console.error(b.id, e);
      if (published) { board.querySelectorAll('.slot').forEach((g) => g.remove()); layout(); }
      document.body.classList.remove('busy'); loading = null; markError(); syncStatus();
      toast(L(`作品生成失败：${errText(e)}`, `Artwork could not be rendered: ${errText(e)}`), 0,
        { acts: [{ label: L('重试', 'Retry'), run: () => void paint() }] });
      return;
    }
    if (token !== state.token) return;
    ring(++done / list.length);
    visible.delete(i);
    if (!a || !preview) {
      slots[i]?.remove();
      layout(published ? board : target); publish();
      continue;
    }
    dims[dimKey(b.id, variant)] = [a.w, a.h];
    arts.set(k, a);
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
    const g = slots[i];
    tile.style.left = g.style.left; tile.style.top = g.style.top; tile.style.width = g.style.width;
    tile.classList.add('placed');
    g.replaceWith(tile);
    if (!published || g.dataset.w !== tile.dataset.w || g.dataset.h !== tile.dataset.h || g.dataset.span !== tile.dataset.span) layout(published ? board : target);
    publish();
    if (published) loaded();
  }
  store.set('dims', JSON.stringify(dims));
  if (batch) styleDone();
  document.body.classList.remove('busy');
  layout();
  // 一张都没画出来也算读完，* 不能一直呼吸
  if (loading) { loading = null; syncStatus(); }
}
/** 风格按钮外圈：已画完的占全部的比例 */
function ring(f: number) {
  const c = document.querySelector<SVGCircleElement>('#dots .ring circle');
  if (c) c.style.strokeDashoffset = String(1 - Math.max(0, Math.min(1, f)));
}
/** 换完风格：外圈淡出，圆点轻弹一下 */
function styleDone() {
  const d = document.querySelector('#dots .cur .d'); if (!d) return;
  d.classList.remove('done'); void (d as HTMLElement).offsetWidth; d.classList.add('done');
}
/** 标签：作品名 · 版式名（和漫游一致），悬停时下面多一行这件作品适合放在哪 */
function labelHtml(id: string, vs: string[], cur?: string, kit = state.style) {
  const q = kit.pieceQ(id, state.lang);
  return `${esc(kit.pieceName(id, state.lang, state.raw))}${vs.length > 1 && cur ? `<span class="vn">${esc(kit.variantName(cur, state.lang))}</span>` : ''}${q ? `<span class="q">${esc(q)}</span>` : ''}`;
}
function tileHtml(id: string, vs: string[], cur?: string) {
  return `<div class="name">${labelHtml(id, vs, cur)}</div>
      <div class="bar glass">${vs.length > 1 ? `<button data-act="prev" title="${L('上一种版式', 'Previous layout')}" aria-label="${L('上一种版式', 'Previous layout')}">${icon('prev', 14)}</button><button data-act="next" title="${L('下一种版式', 'Next layout')}" aria-label="${L('下一种版式', 'Next layout')}">${icon('next', 14)}</button>` : ''}<button data-act="export">${L('导出', 'Export')}</button></div>
      <div class="art"></div>`;
}

/** 瀑布流：先确定缩略图尺寸，再排能容纳的栏数；减栏时不能反过来放大作品。
 *  跨栏的作品会在矮的那栏下面留洞：它挑底下最平的几栏放；几栏高低差超过 40px 就先往后放别的（最多往后看 8 张），
 *  让单栏的作品把矮的那栏垫平再放它。一张跨栏的最多让 6 次，免得宽幅作品全沉到墙底 */
function layout(board: HTMLElement = $('#board')) {
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
    let best = 0, y = Infinity, hole = Infinity;
    for (let c = 0; c + span <= cols; c++) {
      const v = Math.max(...hs.slice(c, c + span));
      let gap = 0; for (let j = c; j < c + span; j++) gap = Math.max(gap, v - hs[j]);
      if (v + gap < y + hole - 1) { y = v; best = c; hole = gap; }
    }
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
  const waited = new Map<HTMLElement, number>();
  const cost = (t: HTMLElement, g: ReturnType<typeof fit>, j: number) => g.y + g.hole + j * 60 // 越往后越不愿意提前，尽量保持原来的顺序
    + (g.span > 1 && g.hole > 40 && (waited.get(t) ?? 0) < 6 ? 2000 : 0);
  while (queue.length) {
    const fits = queue.slice(0, 8).map(fit);
    let pick = 0, f = fits[0], score = cost(queue[0], f, 0);
    for (let j = 1; j < fits.length; j++) {
      const sc = cost(queue[j], fits[j], j);
      if (sc < score - 1) { pick = j; f = fits[j]; score = sc; }
    }
    for (let j = 0; j < pick; j++) if (fits[j].span > 1) waited.set(queue[j], (waited.get(queue[j]) ?? 0) + 1);
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
      const name = tile.querySelector('.name'); if (name) name.innerHTML = labelHtml(id, kit.variants(id), variant, kit);
      const artEl = tile.querySelector('.art') as HTMLElement;
      releasePreview(artEl); artEl.replaceChildren();
      mountPreview(artEl, a, preview); mountRoamHits(artEl, a);
    }
    layout();
  } catch (e) { toast(errText(e), 8000); }
}

// ───────────────────────── 导出 ─────────────────────────
// 墙上、漫游、放大视图用同一个导出菜单：最上面是「复制 CREDITS.md 区块」（不是文件，是个动作），
// 下面格式一列，最底下尺寸三列；记住上次用的格式和尺寸
const closeFormats = () => document.querySelectorAll('.fmts').forEach((x) => { x.parentElement!.classList.remove('menuopen'); x.remove(); });
function toggleFormats(host: HTMLElement, id: string, run: (fmt: string) => void, kit = state.style) {
  const open = host.querySelector(':scope > .fmts');
  closeFormats();
  if (open) return;
  const box = document.createElement('div');
  box.className = 'fmts glass';
  const last = store.get('fmt');
  box.innerHTML = `<button class="act" data-f="md">${icon('copy', 14)}${L('复制 CREDITS.md 区块', 'Copy CREDITS.md block')}</button><i class="rule"></i>`
    + exportsFor(id, kit).filter((f) => f.id !== 'md').map((f) => `<button data-f="${f.id}" class="${f.id === last ? 'last' : ''}">${f.name}</button>`).join('')
    + `<div class="szs" role="group" aria-label="${L('尺寸', 'Size')}">${sizeButtons()}</div>`;
  box.onclick = (e) => {
    e.stopPropagation();
    const b = (e.target as HTMLElement).closest('button'); if (!b) return;
    if (b.dataset.s) { setSize(b.dataset.s); return; }
    closeFormats(); if (b.dataset.f !== 'md') store.set('fmt', b.dataset.f!); run(b.dataset.f!);
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
  if (!o.kit && view() === 'wall' && $('#board').inert) return;
  const kit = o.kit ?? state.style;
  const m = state.model!;
  const raw = state.raw, lang = state.lang, size = state.size;
  const preferences = { group: groupPref(), named: namesOn() };
  const variant = o.kit ? o.variant : curVariant(k);
  const base = `${m.project || 'credits'}-${id}${variant && variant !== 'original' ? '-' + variant : ''}-${kit.id}`;
  try {
    if (fmtId === 'md') {
      const text = mergeCreditsMd(null, creditsBlock(buildText(raw, { lang, ...preferences })));
      try { await navigator.clipboard.writeText(text); return toast(L('CREDITS.md 区块已复制', 'CREDITS.md block copied')); }
      catch { return toast(L('没能写进剪贴板', 'Could not copy to the clipboard'), 8000, { acts: [{ label: L('存成文件', 'Save as a file'), run: () => saveBytes('CREDITS.md', new TextEncoder().encode(text), 'text/markdown').catch((e) => toast(errText(e), 5000)) }] }); }
    }
    toast(fmtId === 'gif' ? L('正在逐帧生成 GIF…', 'Rendering GIF frames…') : L('正在导出…', 'Exporting…'), 60000, { spin: true });
    const a = o.art ?? await drawArt(id, variant, kit);
    if (!a) throw new Error(L('当前档案无法生成这个版式', 'This layout is unavailable for the current project'));
    await new Promise((r) => setTimeout(r, 30));
    const out = await exportArt(fmtId as Format, kit.style, a, size, kit.frames ? () => kit.frames!(raw, lang, id, variant, preferences) : undefined);
    const sz = size !== 'native' && fmtId !== 'html' ? `-${size}` : '';
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
  dock.innerHTML = EXPORTS.filter((f) => f.id !== 'gif' && f.id !== 'md').map((f) => `<button data-f="${f.id}">${f.name}</button>`).join('');
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
const closeBtn = () => `<button class="closer glass" data-act="close" aria-label="${L('关闭', 'Close')}" title="${L('关闭', 'Close')} (Esc)">${icon('close', 14)}</button>`;

function openFocus(id: string, k: string) {
  const el = $('#focus');
  // 放大视图里可以只给这一件换风格试试：kit 是正在看的那一套，墙上的风格不动
  let kit = state.style, art: Art | null = null, preview: Blob | null = null, seq = 0, open = true, exportReady = false;
  const tried: Record<string, string> = {};
  const variantNow = () => kit === state.style ? curVariant(k) : tried[kit.id] ?? kit.current(id);
  const pending = () => {
    exportReady = false; seq++; closeFormats();
    const button = el.querySelector<HTMLButtonElement>('.xp');
    if (button) button.disabled = true;
    el.querySelector('.big')?.classList.add('loading');
  };
  const setVariant = async (v: string) => { pending(); if (kit === state.style) await changeVariant(id, k, v); else tried[kit.id] = v; };
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
      <button class="nav prev glass" data-act="prev" ${at <= 0 ? 'disabled' : ''} title="${L('上一张', 'Previous')} (↑)" aria-label="${L('上一张', 'Previous')}">${icon('prev')}</button>
      <button class="nav next glass" data-act="next" ${at < 0 || at >= all.length - 1 ? 'disabled' : ''} title="${L('下一张', 'Next')} (↓)" aria-label="${L('下一张', 'Next')}">${icon('next')}</button>
      <div class="big"><div class="art"></div></div>
      <div class="strip glass">
        ${vs.length > 1 ? `<div class="vs" title="${L('版式', 'Layout')} (← →)">${vs.map((x) => { const mm = multiModel && MODEL_VARIANTS[id]?.includes(x); return `<button data-v="${x}" class="${x === v ? 'on' : ''}${mm ? ' mm' : ''}">${esc(kit.variantName(x, state.lang))}</button>`; }).join('')}</div><span class="sep"></span>` : ''}
        <div class="pop"><button class="kit" data-act="kits"><svg viewBox="0 0 24 24">${kit.dot}</svg>${esc(kit.name(state.lang))}</button></div>
        ${kit !== state.style ? `<button class="apply" data-act="apply">${L('设为全局风格', 'Use for all cards')}</button>` : ''}
        <span class="sep"></span>
        <div class="pop"><button class="xp" data-act="export" disabled>${L('导出', 'Export')}</button></div>
      </div>`;
    if (art && preview) { mountPreview(el.querySelector('.big .art') as HTMLElement, art, preview); place(art); } // 换风格时旧图先留着，新的画好再换
  };
  const place = (a: Art) => { (el.querySelector('.big') as HTMLElement).style.width = `${Math.max(320, Math.min(a.w, 1100, ((innerHeight - 240) * a.w) / a.h))}px`; };
  // 快速连点几个版式：按钮立刻跟上，图只认最后一次（先点的画得慢，回来也不会盖掉后点的）
  const draw = async () => {
    pending();
    const my = seq, v = variantNow(), k0 = kit;
    el.querySelectorAll<HTMLElement>('.strip button[data-v]').forEach((b) => b.classList.toggle('on', b.dataset.v === v));
    el.querySelector('.vs .on')?.scrollIntoView({ block: 'nearest', inline: 'nearest' }); // 版式多时这一排会横着滚
    try {
      const a = await drawArt(id, v, k0);
      if (my !== seq || !open) return;
      if (!a) throw new Error(L('当前档案无法生成这个版式', 'This layout is unavailable for the current project'));
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
      exportReady = true;
      el.querySelector<HTMLButtonElement>('.xp')!.disabled = false;
    } catch (error) {
      if (my !== seq || !open) return;
      el.querySelector('.big')?.classList.remove('loading');
      toast(errText(error), 8000, { acts: [{ label: L('重试', 'Retry'), run: () => { if (open) void draw(); } }] });
    }
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
  document.body.classList.add('focusing');
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
    if (act === 'export' && exportReady) { el.querySelector('.kits')?.remove(); return toggleFormats(b.parentElement!, id, (f) => { if (exportReady && art) doExport(f, id, k, kit === state.style ? {} : { kit, art, variant: variantNow() }); }, kit); }
    if (b.dataset.v) void setVariant(b.dataset.v).then(() => { if (open) draw(); });
  };
  // 版式已同步到墙上；关闭后键盘焦点回到刚才的位置。
  const close = (refocus = true) => {
    open = false; el.hidden = true; el.innerHTML = ''; closeFocus = null; focusKeys = null;
    document.body.classList.remove('focusing');
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
      el.innerHTML = `<div class="name">${labelHtml(t.id, state.style.variants(t.id), t.v)}</div>
        <div class="bar glass"><button data-act="export">${L('导出', 'Export')}</button></div><div class="art"></div>`;
      const artEl = el.querySelector('.art') as HTMLElement;
      mountRoamPreview(artEl, t.a, t.p, Math.ceil(t.w * devicePixelRatio), previewPriority(el, t));
      mountRoamHits(artEl, t.a);
      (el as any)._item = t;
      plane.appendChild(el);
      roam.els.set(key, el);
      loaded();
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
    setView(b.dataset.v as 'wall' | 'roam');
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
function renderDots() {
  const nav = $('#dots'), t = state.style;
  nav.innerHTML = `<button class="step" data-d="-1">${icon('prev', 14)}</button>`
    + `<button class="cur" id="curstyle" aria-haspopup="listbox"><span class="d"><svg viewBox="0 0 24 24">${t.dot}</svg><svg class="ring" viewBox="0 0 26 26" aria-hidden="true"><circle cx="13" cy="13" r="12" pathLength="1"/></svg></span>${esc(t.name(state.lang))}${icon('updown', 12, 'chev')}</button>`
    + `<button class="step" data-d="1">${icon('next', 14)}</button>`;
  const [prev, next] = nav.querySelectorAll<HTMLElement>('.step');
  prev.title = L('上一种风格', 'Previous style'); next.title = L('下一种风格', 'Next style');
  prev.setAttribute('aria-label', prev.title); next.setAttribute('aria-label', next.title);
  nav.onclick = (e) => {
    const b = (e.target as HTMLElement).closest('button') as HTMLElement | null; if (!b) return;
    if (b.dataset.d) return void stepStyle(Number(b.dataset.d));
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
async function useStyle(t: Kit, turn = 1) {
  if (t.id === state.style.id) return;
  state.style = t; store.set('theme', t.id);
  state.variants = {};
  renderDots();
  mark.turn(turn); // 和风格按钮呼应：* 跟着转一瓣
  await paint();
}
const stepStyle = (d: number) => { const i = KITS.indexOf(state.style); return useStyle(KITS[(i + d + KITS.length) % KITS.length], d); };

// ───────────────────────── 顶栏：项目、视图、设置 ─────────────────────────
/** 页面里写死的字、无障碍标签、<html lang> */
function applyLang() {
  document.documentElement.lang = state.lang === 'zh' ? 'zh-CN' : 'en';
  const gear = $('#gear');
  gear.title = `${L('设置', 'Settings')} (${MOD},)`; gear.setAttribute('aria-label', L('设置', 'Settings'));
  $('#project').setAttribute('aria-label', L('切换项目', 'Switch project'));
  $('#project').title = `${L('切换项目', 'Switch project')} (${MOD}K)`;
  $('#dots').setAttribute('aria-label', L('风格', 'Styles'));
  $('#dock').setAttribute('aria-label', L('导出', 'Export'));
  const [w, r] = document.querySelectorAll<HTMLElement>('#view button');
  w.setAttribute('aria-label', L('墙', 'Wall')); r.setAttribute('aria-label', L('漫游', 'Roam'));
  w.title = `${L('墙', 'Wall')} (${MOD}1)`; r.title = `${L('漫游', 'Roam')} (${MOD}2)`;
  $('#view').setAttribute('aria-label', L('视图', 'View'));
  $('#home').innerHTML = `${icon('home', 14)}${L('回到起点', 'Back to start')}`;
}
function wireTop() {
  const pill = $('#project');
  pill.onclick = () => toggleMenu();
  pill.onkeydown = (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); toggleMenu(); } };
  addEventListener('keydown', (e) => {
    if (e.key === 'Escape') return closeTop();
    if (focusKeys && $('#sheet').hidden && focusKeys(e)) e.preventDefault();
  });
  const t = $('#toast');
  t.onmouseenter = () => clearTimeout(toastTimer);
  t.onmouseleave = () => { if (toastMs > 0) toastTimer = setTimeout(() => (t.hidden = true), 1500); };
  $('#gear').onclick = () => settings();
}
/** 换界面语言：顶栏、风格条、导出坞、菜单栏、墙上的字一起换 */
async function setLang(l: 'zh' | 'en') {
  if (l === state.lang) return;
  state.lang = l; store.set('lang', l);
  applyLang(); renderDock(); renderDots(); closeMenu(); void installMenu();
  if (!$('#sheet').hidden && sheetRedraw) sheetRedraw();
  welcomeDraw?.();
  syncStatus();
  if (state.raw) { rebuildModel(); await paint(); }
  else if (emptyView && !$('#empty').hidden) empty(emptyView, !!state.current, emptyErr);
}
function setView(v: 'wall' | 'roam') {
  store.set('view', v); applyView();
  if (view() === 'wall' && wallStale) void paint();
}

// ───────────────────────── 快捷键、菜单栏 ─────────────────────────
// 同一张表：macOS 上装进系统菜单栏（菜单里看得到快捷键），别的系统直接听键盘，不在窗口上加一条菜单栏
type Cmd = { id: string; key: string; label: () => string; run: () => unknown };
const canAnalyze = (p: any) => !!p && !p.all && p.exists !== false && !analyzing;
const CMDS: Cmd[] = [
  { id: 'switch', key: 'K', label: () => L('切换项目…', 'Switch Project…'), run: () => toggleMenu() },
  { id: 'add', key: 'O', label: () => L('添加文件夹…', 'Add Folder…'), run: () => { closeMenu(); return addProject(); } },
  { id: 'reanalyze', key: 'R', label: () => L('重新分析', 'Re-analyze'), run: () => { if (canAnalyze(state.current)) return reanalyze(state.current); } },
  { id: 'project', key: 'I', label: () => L('项目设置…', 'Project Settings…'), run: () => { if (state.current && !state.current.all) projectSettings(state.current); } },
  { id: 'settings', key: ',', label: () => L('设置…', 'Settings…'), run: () => settings() },
  { id: 'wall', key: '1', label: () => L('墙', 'Wall'), run: () => setView('wall') },
  { id: 'roam', key: '2', label: () => L('漫游', 'Roam'), run: () => setView('roam') },
  { id: 'prevstyle', key: '[', label: () => L('上一种风格', 'Previous Style'), run: () => stepStyle(-1) },
  { id: 'nextstyle', key: ']', label: () => L('下一种风格', 'Next Style'), run: () => stepStyle(1) },
  { id: 'export', key: 'E', label: () => L('导出…', 'Export…'), run: () => exportHere() },
];
/** ⌘E：放大视图里导出那一张；墙上导出键盘停着或鼠标指着的那一张 */
function exportHere() {
  const fb = document.querySelector<HTMLElement>('#focus:not([hidden]) [data-act="export"]');
  if (fb) return fb.click();
  const tile = document.querySelector<HTMLElement>('#board .tile:not(.slot):focus, #board .tile:not(.slot):hover, #roam .rt:hover');
  if (tile?.closest('[inert]')) return;
  tile?.querySelector<HTMLElement>('.bar [data-act="export"]')?.click();
}
let menuKeys = false;
function wireKeys() {
  addEventListener('keydown', (e) => {
    if (menuKeys || e.altKey || e.shiftKey || !(OS === 'mac' ? e.metaKey : e.ctrlKey)) return;
    const c = CMDS.find((x) => x.key === e.key.toUpperCase());
    if (!c || !$('#welcome').hidden && c.id !== 'add' && c.id !== 'settings') return;
    e.preventDefault(); e.stopPropagation();
    void c.run();
  }, true);
}
/** macOS 的菜单栏：应用、编辑（输入框要用复制粘贴）、项目、显示、窗口。换语言时重建 */
async function installMenu() {
  const T = (window as any).__TAURI__?.menu;
  if (!T || OS !== 'mac') return;
  try {
    const { Menu, Submenu, MenuItem, PredefinedMenuItem } = T;
    const sep = () => PredefinedMenuItem.new({ item: 'Separator' });
    const pre = (item: string) => PredefinedMenuItem.new({ item });
    const cmd = (id: string) => { const c = CMDS.find((x) => x.id === id)!; return MenuItem.new({ id, text: c.label(), accelerator: `CmdOrCtrl+${c.key}`, action: () => void c.run() }); };
    const sub = async (text: string, items: Promise<unknown>[]) => Submenu.new({ text, items: await Promise.all(items) });
    const menu = await Menu.new({ items: await Promise.all([
      sub('creditia', [PredefinedMenuItem.new({ item: { About: null } }), sep(), cmd('settings'), sep(), pre('Services'), sep(), pre('Hide'), pre('HideOthers'), pre('ShowAll'), sep(), pre('Quit')]),
      sub(L('编辑', 'Edit'), [pre('Undo'), pre('Redo'), sep(), pre('Cut'), pre('Copy'), pre('Paste'), pre('SelectAll')]),
      sub(L('项目', 'Project'), [cmd('switch'), cmd('add'), sep(), cmd('reanalyze'), cmd('project')]),
      sub(L('显示', 'View'), [cmd('wall'), cmd('roam'), sep(), cmd('prevstyle'), cmd('nextstyle'), sep(), cmd('export'), sep(), pre('Fullscreen')]),
      sub(L('窗口', 'Window'), [pre('Minimize'), pre('Maximize'), sep(), pre('CloseWindow'), pre('BringAllToFront')]),
    ]) });
    await menu.setAsAppMenu();
    menuKeys = true;
  } catch (e) { console.warn('menu', e); menuKeys = false; }
}

// ───────────────────────── 项目切换 ─────────────────────────
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
/** 项目菜单只管挑项目：搜索、全部项目、各项目（右边「…」是这个项目能做的事）、还没添加的；底下只有添加文件夹 */
function toggleMenu() {
  const menu = $('#menu'), pill = $('#project');
  if (!menu.hidden) return closeMenu();
  closeFocus?.(); closeSheet();
  pill.setAttribute('aria-expanded', 'true');
  menu.style.left = `${Math.max(16, Math.round(pill.getBoundingClientRect().left))}px`; // 对齐项目名，Mac 上让开红绿灯
  menu.hidden = false;
  menu.innerHTML = `<label class="q">${icon('search', 14)}<input id="q" placeholder="${L('搜索项目', 'Search projects')}" autocomplete="off" spellcheck="false"></label>
    <div class="list" id="plist" role="listbox" aria-label="${L('项目', 'Projects')}"></div>
    <div class="foot"><button id="add" ${analyzing ? 'disabled' : ''}>${icon('addFolder')}${L('添加文件夹…', 'Add folder…')}<kbd>${MOD}O</kbd></button></div>`;
  const day = (t: number) => new Date(t * 1000).toLocaleDateString(state.lang === 'zh' ? 'zh-CN' : 'en-US');
  const more = (id: string) => `<button class="more" data-more="${esc(id)}" aria-haspopup="menu" aria-expanded="false" aria-label="${L('更多', 'More')}" title="${L('更多', 'More')}">${icon('more')}</button>`;
  const draw = () => {
    const q = ($('#q') as HTMLInputElement).value.trim().toLowerCase();
    const hit = (s: string) => !q || s.toLowerCase().includes(q);
    const all = state.projects.length > 1 && (hit(L('全部项目', 'All projects')) || 'all'.includes(q))
      ? `<div class="item all ${state.current?.all ? 'on' : ''}" data-id="${ALL_ID}" role="option" aria-selected="${!!state.current?.all}">${markEl(markOf(ALL_ID) ?? UNREAD)}<b>${L('全部项目', 'All projects')}</b><span class="r">${L(`${state.projects.length} 个`, `${state.projects.length}`)}</span></div>` : '';
    const mine = state.projects.filter((p) => hit(`${p.name} ${p.path}`)).map((p) => `
      <div class="item ${p.id === state.current?.id ? 'on' : ''} ${p.exists === false ? 'gone' : ''}" data-id="${esc(p.id)}" role="option" aria-selected="${p.id === state.current?.id}">
        ${markEl(markOf(p.id) ?? UNREAD)}<b>${esc(p.name)}</b><span class="r">${p.exists === false ? L('目录不在了', 'folder missing') : p.lastAnalyzed ? day(p.lastAnalyzed) : ''}</span>${more(p.id)}
        <span class="path" title="${esc(p.path)}">${esc(short(p.path))}</span>
      </div>`).join('');
    // 本机用过 AI 工具、还没加进来的项目：点一下就分析
    const fresh = (found ?? []).filter((d) => hit(`${d.name} ${d.path}`)).map((d) => `
      <div class="item new" data-path="${esc(d.path)}" role="option" aria-selected="false">
        ${markEl(UNREAD)}<b>${esc(d.name)}</b><span class="r">${esc(toolsOf(d))}</span>
        <span class="path" title="${esc(d.path)}">${esc(short(d.path))}</span>
      </div>`).join('');
    $('#plist').innerHTML = all + mine + (fresh ? `<div class="sec">${L('还没添加', 'Not added yet')}</div>${fresh}` : '')
      || `<div class="item"><span>${L('没有匹配的项目', 'No match')}</span></div>`;
  };
  // ↑ ↓ 选，回车打开（没选就开第一个）
  let kb = -1;
  const items = () => [...$('#plist').querySelectorAll<HTMLElement>('.item[data-id], .item[data-path]')];
  const markKb = () => items().forEach((x, j) => { x.classList.toggle('kb', j === kb); if (j === kb) x.scrollIntoView({ block: 'nearest' }); });
  draw();
  if (!demo && found === null) void discover().then(() => { if (!menu.hidden) draw(); });
  void fillMarks(() => { if (!menu.hidden) draw(); });
  const q = $('#q') as HTMLInputElement;
  q.oninput = () => { kb = -1; closeRowMenu(); draw(); };
  q.onkeydown = (e) => {
    const n = items().length;
    if (e.key === 'ArrowDown' && n) { kb = (kb + 1) % n; markKb(); e.preventDefault(); }
    else if (e.key === 'ArrowUp' && n) { kb = (kb - 1 + n) % n; markKb(); e.preventDefault(); }
    else if (e.key === 'Enter' && n) { items()[Math.max(kb, 0)].click(); e.preventDefault(); }
  };
  q.focus();
  $('#plist').onclick = (e) => {
    const m = (e.target as HTMLElement).closest<HTMLElement>('[data-more]');
    if (m) { e.stopPropagation(); return rowMenu(m); }
    const it = (e.target as HTMLElement).closest('.item') as HTMLElement | null; if (!it) return;
    if (it.dataset.path) { closeMenu(); return void addProjects([it.dataset.path]); }
    if (!it.dataset.id) return;
    closeMenu();
    if (it.dataset.id === ALL_ID) { if (!state.current?.all) openProject(allProject()); return; }
    const p = state.projects.find((x) => x.id === it.dataset.id);
    if (p && p !== state.current) openProject(p);
  };
  $('#plist').onscroll = closeRowMenu;
  $('#add').onclick = () => { closeMenu(); addProject(); };
  setTimeout(() => document.addEventListener('click', function off(e) { if (!menu.contains(e.target as Node) && !pill.contains(e.target as Node)) { closeMenu(); document.removeEventListener('click', off); } }), 0);
}
function closeRowMenu() {
  document.querySelector('.rowmenu')?.remove();
  document.querySelectorAll('[data-more][aria-expanded="true"]').forEach((b) => b.setAttribute('aria-expanded', 'false'));
}
/** 一行的「…」：重新分析、在访达中显示、项目设置、移除。挂在菜单上而不是列表里，列表滚动不会把它裁掉 */
function rowMenu(btn: HTMLElement) {
  const open = btn.getAttribute('aria-expanded') === 'true';
  closeRowMenu();
  if (open) return;
  const p = state.projects.find((x) => x.id === btn.dataset.more); if (!p) return;
  const menu = $('#menu'), box = document.createElement('div');
  box.className = 'rowmenu glass'; box.setAttribute('role', 'menu');
  const gone = p.exists === false;
  box.innerHTML = `<button data-r="re" role="menuitem" ${canAnalyze(p) ? '' : 'disabled'}>${icon('refresh')}${L('重新分析', 'Re-analyze')}${p === state.current ? `<kbd>${MOD}R</kbd>` : ''}</button>
    <button data-r="open" role="menuitem" ${gone || !native ? 'disabled' : ''}>${icon('folder')}${esc(revealLabel())}</button>
    <button data-r="settings" role="menuitem">${icon('info')}${L('项目设置…', 'Project settings…')}${p === state.current ? `<kbd>${MOD}I</kbd>` : ''}</button>
    <i class="sep"></i>
    <button data-r="remove" role="menuitem">${icon('remove')}${L('从作品库移除…', 'Remove from library…')}</button>`;
  menu.appendChild(box);
  const mr = menu.getBoundingClientRect(), br = btn.getBoundingClientRect();
  const below = br.bottom - mr.top + 2;
  box.style.top = below + box.offsetHeight > innerHeight - mr.top - 8 ? `${br.top - mr.top - box.offsetHeight - 2}px` : `${below}px`;
  box.style.right = `${Math.round(mr.right - br.right)}px`;
  btn.setAttribute('aria-expanded', 'true');
  box.onclick = (e) => {
    e.stopPropagation();
    const b = (e.target as HTMLElement).closest<HTMLButtonElement>('button[data-r]'); if (!b || b.disabled) return;
    closeRowMenu(); closeMenu();
    const r = b.dataset.r;
    if (r === 're') void reanalyze(p);
    else if (r === 'open') api('open_folder', { which: 'project', projectId: p.id }).catch((err) => toast(errText(err), 5000));
    else projectSettings(p, r === 'remove');
  };
}
/** 从作品库里拿掉：只删列表里这一项，分析记录和缓存都留着，加回来不用从头分析 */
async function removeProject(p: any) {
  try { await api('remove_project', { projectId: p.id }); }
  catch (e) { markError(); return toast(errText(e), 6000); }
  state.projects = await api('projects').catch(() => state.projects.filter((x) => x.id !== p.id));
  found = null; finding = null; // 它又是「还没添加」的了
  const acts = p.exists !== false && native ? [{ label: L('加回来', 'Add back'), run: () => void addProjects([p.path]) }] : [];
  toast(L(`已移除 ${p.name}，分析记录仍留在本机`, `Removed ${p.name} — its analysis stays on this computer`), 8000, { acts });
  if (state.current?.id === p.id || state.current?.all) {
    const next = state.projects[0];
    if (next) await openProject(next);
    else { state.current = null; state.raw = null; state.model = null; $('#board').innerHTML = ''; $('#pname').textContent = 'creditia'; welcome(); }
  }
}

// ───────────────────────── 本机的项目 ─────────────────────────
// 核心的 discover 扫各工具的记录，列出用过 AI 工具的仓库。第一次打开和「添加」时用，一次会话扫一遍
type Found = { name: string; path: string; tools: Record<string, number>; lastActive: string; exists: boolean; inLibrary: boolean };
let found: Found[] | null = null, finding: Promise<Found[]> | null = null;
function discover(): Promise<Found[]> {
  finding ??= api('discover').then((r: any) => (found = ((r?.projects ?? []) as Found[]).filter((d) => d.exists && !state.projects.some((p) => p.path === d.path))), () => (found = []));
  return finding;
}
const toolsOf = (d: Found) => Object.entries(d.tools).sort((a, b) => b[1] - a[1]).slice(0, 2).map(([t]) => nameOf(t, state.lang)).join(' · ');

// ───────────────────────── 分析：进度、取消 ─────────────────────────
// 进度交给 *：灰的五瓣按进度一瓣瓣长满（核心报来的四步各占一段），程序坞上同步一条进度条；
// 挡在前面（没有墙可看）时 * 在中间，否则在左上角。取消按钮跟着那一句话
let analyzing = false;
/** 核心的四步：名字和在整条进度里占的一段 */
const STAGES: Record<string, [string, string, number, number]> = {
  history: ['读提交历史', 'Reading history', 0, 0.1], logs: ['读各工具的日志', 'Reading tool logs', 0.1, 0.3],
  attribute: ['逐个提交归因', 'Attributing commits', 0.3, 0.85], blame: ['追溯现在的每个文件', 'Tracing current files', 0.85, 1],
};
/** 浏览器里没有核心：开发时按同样的四步假装跑一遍，看得到进度的样子 */
let devProgress: ((p: any) => void) | null = null, devCancel = false;
async function devAnalyze() {
  devCancel = false;
  for (const [stage, total, ms] of [['history', 0, 500], ['logs', 6, 160], ['attribute', 40, 70], ['blame', 12, 90]] as const) {
    for (let done = 0; done <= Math.max(1, total); done++) {
      if (devCancel) throw { code: 'cancelled' };
      devProgress?.({ stage, done, total });
      await new Promise((r) => setTimeout(r, ms));
    }
  }
}
async function withProgress<T>(name: string, run: () => Promise<T>, o: { auto?: boolean; blocking?: boolean } = {}): Promise<T> {
  if (analyzing) throw { code: 'busy' };
  analyzing = true;
  const my: Job = { name, auto: !!o.auto, blocking: !!o.blocking, t0: Date.now(), stage: '', p: null };
  job = my; markErr = false;
  mark.progress(0);
  if (my.auto) mark.kick(0); // 后台跟上：只轻跳一下，不变灰
  syncStatus();
  const onProgress = (p: any) => {
    const n = STAGES[p?.stage]; if (!n || job !== my) return;
    my.stage = `${L(n[0], n[1])}${p.total ? ` ${fmt(p.done)}/${fmt(p.total)}` : ''}`;
    my.p = n[2] + (n[3] - n[2]) * (p.total ? Math.min(1, p.done / p.total) : 0);
    syncStatus();
  };
  devProgress = native ? null : onProgress;
  const unlisten: (() => void) | null = await (window as any).__TAURI__?.event?.listen('analyze-progress', (e: any) => onProgress(e.payload)).catch(() => null) ?? null;
  try { return await run(); }
  finally { unlisten?.(); devProgress = null; analyzing = false; if (job === my) job = null; syncStatus(); }
}
async function reanalyze(p: any, auto = false) {
  // 没有墙可看（空状态里点「现在分析」）：空状态让开，* 到中间
  const seq = state.openSeq;
  const current = () => seq === state.openSeq && state.current?.id === p.id;
  const blocking = !auto && current() && !painted();
  const wasEmpty = blocking && !$('#empty').hidden && emptyView;
  if (wasEmpty) $('#empty').hidden = true;
  try {
    const v: any = await withProgress(p.name, () => api('analyze', { projectId: p.id, lang: state.lang }), { auto, blocking });
    state.projects = await api('projects');
    const fresh = state.projects.find((x) => x.id === p.id) ?? p;
    // 分析归属于启动时的项目，用户切走后不改变当前视图。
    if (current()) {
      if (state.raw) {
        const st = readStamp(fresh);
        const raw = await api('credits', { projectId: p.id });
        const stamp = await st;
        if (current()) {
          state.current = fresh; stampOf = stamp;
          await swapRaw(raw);
        }
      } else await openProject(fresh);
    }
    if (auto && !current()) return;
    const n = Number(v?.newCommits) || 0;
    if (auto) flash(n ? L(`已跟上 ${n} 个新提交`, `Caught up with ${n} new commit${n > 1 ? 's' : ''}`) : L('已更新', 'Updated'));
    else done(current() ? L('已更新', 'Updated') : L(`${p.name} 已更新`, `${p.name} updated`), v?.warnings);
  } catch (e: any) {
    if (auto && !current()) return;
    if (current() && wasEmpty && !state.raw) empty(wasEmpty, true, emptyErr);
    const message = current() ? errText(e) : `${p.name}: ${errText(e)}`;
    if (e?.code === 'cancelled') return flash(message);
    if (auto && e?.code === 'busy') return;
    if (current()) markError();
    toast(message, 6000);
  }
}
async function addProject() {
  let dir: string | null;
  try { dir = await api('pick_folder'); } catch (e) { return toast(errText(e), 5000); }
  if (dir) await addProjects([dir]);
}
/** 一个接一个分析、加进作品库。第一个分析完就打开它，其余的在后台接着跑（* 上看进度、能取消） */
async function addProjects(paths: string[], each?: (path: string, st: 'run' | 'done' | 'fail') => void) {
  let opened = false, ok = 0;
  const notes: string[] = [];
  for (const [i, path] of paths.entries()) {
    const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
    each?.(path, 'run');
    try {
      const r = await withProgress(paths.length > 1 ? L(`${name}（${i + 1}/${paths.length}）`, `${name} (${i + 1}/${paths.length})`) : name, () => api('add_project', { path, lang: state.lang }), { blocking: !painted() });
      state.projects = await api('projects'); ok++;
      if (found) found = found.filter((d) => d.path !== path);
      notes.push(...(r?.result?.warnings ?? []));
      each?.(path, 'done');
      const p = state.projects.find((x) => x.id === r?.id);
      if (p && !opened) { opened = true; $('#welcome').hidden = true; welcomeDraw = null; void openProject(p); }
    } catch (e: any) {
      each?.(path, 'fail');
      if (e?.code === 'cancelled') { flash(errText(e)); return; }
      markError();
      toast(L(`${name}：${errText(e)}`, `${name}: ${errText(e)}`), 6000);
    }
  }
  if (ok) done(ok > 1 ? L(`已添加 ${ok} 个项目`, `Added ${ok} projects`) : L('已添加', 'Added'), notes);
}

// ───────────────────────── 第一次打开 ─────────────────────────
// 作品库是空的：列出本机用过 AI 工具的仓库，勾几个就开始分析；没找到就选文件夹。
// * 在标题上面：扫描时呼吸，分析时一瓣瓣长满，下面一行写着走到哪一步
let welcomeDraw: (() => void) | null = null;
function welcome() {
  const el = $('#welcome');
  el.hidden = false; $('#empty').hidden = true; $('#board').innerHTML = '';
  applyView();
  const picked = new Set<string>();
  let list: Found[] | null = null;
  const status = new Map<string, string>();
  const ST: Record<string, string> = { run: SPIN(), done: icon('check', 14), fail: icon('close', 14) };
  const draw = () => {
    if (el.hidden) return;
    const running = [...status.values()].includes('run');
    el.innerHTML = `<div class="box">
      <span class="logo"></span>
      <h1>${L('选择要分析的项目', 'Select projects to start')}</h1>
      <p class="wstat" aria-live="polite"></p>
      <div class="found">${list === null ? `<p class="wait">${L('正在扫描本地包含 AI 工具记录的 Git 仓库…', 'Looking for repos where you used AI tools…')}</p>`
        : list.length ? list.map((d) => `<label class="row ${status.get(d.path) ?? ''}" data-path="${esc(d.path)}">
            <input type="checkbox" ${picked.has(d.path) ? 'checked' : ''} ${running || status.has(d.path) ? 'disabled' : ''}>
            <b>${esc(d.name)}</b><span class="path" title="${esc(d.path)}">${esc(short(d.path))}</span><span class="tools">${esc(toolsOf(d))}</span>
            <i class="st">${ST[status.get(d.path) ?? ''] ?? ''}</i></label>`).join('')
        : `<p class="wait">${L('未发现包含 AI 工具记录的 Git 仓库', 'No git repos with AI tool activity found')}</p>`}</div>
      <div class="acts"><button class="pick" ${running ? 'disabled' : ''}>${icon('folder')}${L('选择文件夹…', 'Choose folder…')}</button>${list?.length ? `<button class="go" ${!picked.size || running ? 'disabled' : ''}>${running ? L('分析中…', 'Analyzing…') : picked.size > 1 ? L(`分析这 ${picked.size} 个`, `Analyze ${picked.size}`) : L('开始分析', 'Analyze')}</button>` : ''}</div>
    </div>`;
    settleMark(); syncStatus();
  };
  welcomeDraw = draw;
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
  mark.set(BRAND, { grow: true });
  scanning = true;
  draw();
  void discover().then((r) => { scanning = false; list = r; r.slice(0, 3).forEach((d) => picked.add(d.path)); draw(); });
}

/** 分析跑完：一句结果写在项目名后面；核心有提醒（某个数据源读不了之类）就再给个能点「查看」的提示 */
function done(msg: string, warnings?: string[]) {
  if (!warnings?.length) return flash(msg);
  const text = L(`${msg} · ${warnings.length} 条提醒`, `${msg} · ${warnings.length} ${warnings.length > 1 ? 'notes' : 'note'}`);
  flash(text, 5000);
  toast(text, 10000, { acts: [{ label: L('查看', 'View'), run: () => showWarnings(warnings) }] });
}
function showWarnings(list: string[]) {
  const el = $('#sheet');
  el.hidden = false; sheetRedraw = null;
  el.innerHTML = `<div class="panel glass">
    <div class="head"><h2>${L('分析时遇到的问题', 'Issues during analysis')}</h2>${closeBtn()}</div>
    <ul class="warn">${list.map((w) => `<li>${esc(w)}</li>`).join('')}</ul>
  </div>`;
  el.onclick = (e) => { if (e.target === el || (e.target as HTMLElement).closest('[data-act="close"]')) closeSheet(); };
}

function closeSheet() { const el = $('#sheet'); el.hidden = true; el.innerHTML = ''; sheetRedraw = null; plugProject = null; }
/** 开着的设置页换语言时按新语言重画 */
let sheetRedraw: (() => void) | null = null;
const logoNow = () => (store.get('logo') && LOGOS[store.get('logo')!] ? store.get('logo')! : DEFAULT_LOGO);
const dayOf = (t: number) => new Date(t * 1000).toLocaleString(state.lang === 'zh' ? 'zh-CN' : 'en-US', { dateStyle: 'medium', timeStyle: 'short' });

// ───────────────────────── 项目设置 ─────────────────────────
// 一个项目自己的事都在这里：分析、有新提交时要不要自动跟上、两项接入、关联的路径、从作品库移除
function projectSettings(p0: any, armRemove = false) {
  closeMenu();
  const el = $('#sheet');
  el.hidden = false;
  let p = p0, armed = armRemove, busy = '', plugFailed = false;
  plugProject = p; plugs = null;
  const draw = () => {
    const own = autoOwn(p.id), paths: string[] = [...new Set([p.path, ...(p.paths ?? [])])];
    const seg = ([['', L(`跟随默认（${autoDefault() ? '开' : '关'}）`, `Default (${autoDefault() ? 'on' : 'off'})`)], ['on', L('自动跟上', 'Catch up')], ['off', L('不自动', 'Off')]] as const)
      .map(([v, n]) => `<button data-x="auto" data-v="${v}" role="radio" aria-checked="${(own ?? '') === v}" class="${(own ?? '') === v ? 'on' : ''}">${esc(n)}</button>`).join('');
    el.innerHTML = `<div class="panel glass" role="dialog" aria-label="${L('项目设置', 'Project settings')}">
      <div class="head"><span class="pm">${markEl(markOf(p.id) ?? UNREAD)}</span><div class="who"><h2>${esc(p.name)}</h2><small title="${esc(p.path)}">${esc(short(p.path))}</small></div>${closeBtn()}</div>
      <h3>${L('分析', 'Analysis')}</h3>
      <div class="row kv"><span class="k">${L('上次分析', 'Last analyzed')}</span><span class="v grow">${p.exists === false ? L('项目目录已经不在了', 'The project folder is gone') : p.lastAnalyzed ? esc(dayOf(p.lastAnalyzed)) : L('还没分析过', 'Not analyzed yet')}</span><button class="mini" data-x="re" ${canAnalyze(p) ? '' : 'disabled'}>${icon('refresh', 14)}${L('重新分析', 'Re-analyze')}</button></div>
      <div class="row kv"><span class="k">${L('有新提交时', 'On new commits')}</span><span class="v grow"></span><div class="seg" role="radiogroup" aria-label="${L('有新提交时', 'On new commits')}">${seg}</div></div>
      <h3>${L('接入', 'Integrations')}</h3>
      <div class="mplugs" id="mplugs" hidden></div>
      <p class="note" id="plugwait">${p.exists === false ? L('项目目录不在了，接入暂时改不了', "The folder is gone, so integrations can't be changed") : plugFailed ? `${L('读不到接入的状态', "Couldn't read the integrations")} <button class="mini" data-x="plugs-retry">${L('重试', 'Retry')}</button>` : L('读取中…', 'Loading…')}</p>
      <h3>${L('路径', 'Paths')}</h3>
      <p>${L('日志里这些路径下的改动也算这个项目：worktree、搬家前的旧目录、别处的克隆。', 'Changes under these paths in tool logs count for this project: worktrees, old locations, other clones.')}</p>
      ${paths.map((x) => `<div class="row"><span class="v path" title="${esc(x)}">${esc(short(x))}</span>${x === p.path ? `<span class="tag">${L('主目录', 'Main')}</span>` : p.pathKinds?.[x] === 'manual' ? `<button class="mini" data-x="unlink" data-path="${esc(x)}" ${busy ? 'disabled' : ''}>${L('取消关联', 'Unlink')}</button>` : `<span class="tag">${L('自动发现', 'Auto-detected')}</span>`}</div>`).join('')}
      <div class="row"><button class="mini" data-x="link" ${busy || p.exists === false ? 'disabled' : ''}>${icon('link', 14)}${L('关联一个文件夹…', 'Link a folder…')}</button></div>
      <h3>${L('作品库', 'Library')}</h3>
      <div class="row"><span class="k">${L('从作品库移除', 'Remove from library')}</span><button class="mini danger${armed ? ' armed' : ''}" data-x="remove">${icon('remove', 14)}${armed ? L('确认移除', 'Confirm remove') : L('移除', 'Remove')}</button></div>
      <p class="note">${L('只从列表里拿掉。分析记录和缓存都留在本机，再加回来不用从头分析；仓库里的文件不会动。', "Only takes it off the list. Its analysis and caches stay on this computer, so adding it back is quick; nothing in the repo changes.")}</p>
    </div>`;
    renderPlugs();
    if (plugs) el.querySelector('#plugwait')?.remove();
  };
  sheetRedraw = draw;
  draw();
  const readPlugs = async () => {
    plugFailed = false; draw();
    await loadPlugs(p);
    if (plugProject === p && sheetRedraw === draw) { plugFailed = !plugs; draw(); }
  };
  if (p.exists !== false && !p.all) void readPlugs();
  const refresh = async () => { state.projects = await api('projects').catch(() => state.projects); p = state.projects.find((x) => x.id === p.id) ?? p; if (sheetRedraw === draw) plugProject = p; if (state.current?.id === p.id) state.current = p; };
  el.onclick = async (e) => {
    const t = e.target as HTMLElement;
    if (e.target === el || t.closest('[data-act="close"]')) return closeSheet();
    const b = t.closest<HTMLButtonElement>('[data-x]'); if (!b || b.disabled) return;
    const x = b.dataset.x;
    if (x === 'plugs-retry') return void readPlugs();
    if (x !== 'remove' && armed) { armed = false; draw(); }
    try {
      if (x === 're') { closeSheet(); return void reanalyze(p); }
      if (x === 'auto') {
        store.set('autofresh.' + p.id, b.dataset.v!);
        draw();
        if (autoFresh(p) && state.current?.id === p.id) void refreshIfChanged();
      } else if (x === 'remove') {
        if (!armed) { armed = true; return draw(); }
        closeSheet(); return void removeProject(p);
      } else if (x === 'link' || x === 'unlink') {
        const path = x === 'link' ? await api('pick_folder') : b.dataset.path;
        if (!path) return;
        busy = x; draw();
        const r = await api('link_path', { projectId: p.id, path, unlink: x === 'unlink', lang: state.lang });
        await refresh();
        if (r?.summary) toast(String(r.summary), 6000);
      }
    } catch (err) { markError(); toast(errText(err), 6000); }
    finally { if (busy) { busy = ''; if (sheetRedraw === draw) draw(); } }
  };
}

// ───────────────────────── 设置 ─────────────────────────
// 左边四栏：通用（语言、标志、自动跟上的默认）、导出、数据（项目库、备份、AI 工具）、关于
type Tab = 'general' | 'export' | 'data' | 'about';
let settingsTab: Tab = 'general';
async function settings(tab: Tab = settingsTab) {
  closeMenu();
  settingsTab = tab;
  const el = $('#sheet');
  el.hidden = false; plugProject = null;
  sheetRedraw = () => void settings(settingsTab);
  const tabs: [Tab, string, string][] = [['general', 'gear', L('通用', 'General')], ['export', 'export', L('导出', 'Export')], ['data', 'folder', L('数据', 'Data')], ['about', 'info', L('关于', 'About')]];
  el.innerHTML = `<div class="panel glass wide" role="dialog" aria-label="${L('设置', 'Settings')}">
    <nav class="nav"><h2>${L('设置', 'Settings')}</h2>${tabs.map(([id, ic, n]) => `<button data-tab="${id}" class="${id === tab ? 'on' : ''}" aria-current="${id === tab}">${icon(ic)}${esc(n)}</button>`).join('')}</nav>
    <div class="body">${closeBtn()}<div class="sec"></div></div>
  </div>`;
  const sec = el.querySelector('.sec') as HTMLElement;
  const live = () => sec.isConnected; // 换了一栏，慢的请求回来就不写了
  el.onclick = (e) => {
    const t = e.target as HTMLElement;
    if (e.target === el || t.closest('[data-act="close"]')) return closeSheet();
    const tb = t.closest<HTMLElement>('[data-tab]'); if (tb) return void settings(tb.dataset.tab as Tab);
  };
  if (tab === 'general') {
    sec.innerHTML = `<h3>${L('通用', 'General')}</h3>
      <div class="row kv"><span class="k">${L('语言', 'Language')}</span><span class="v grow"></span><div class="seg" role="radiogroup" aria-label="${L('语言', 'Language')}">${([['zh', '中文'], ['en', 'English']] as const).map(([v, n]) => `<button data-lang="${v}" role="radio" aria-checked="${state.lang === v}" class="${state.lang === v ? 'on' : ''}">${n}</button>`).join('')}</div></div>
      <div class="row kv"><span class="k">${L('程序坞图标', 'App icon')}</span><span class="v grow"></span><div class="logos" role="radiogroup" aria-label="${L('程序坞图标', 'App icon')}">${Object.entries(LOGOS).map(([id, g]) => `<button data-logo="${id}" role="radio" aria-checked="${id === logoNow()}" title="${esc(L(...g.name))}" aria-label="${esc(L(...g.name))}">${g.svg}</button>`).join('')}</div></div>
      <div class="sw${autoDefault() ? ' on' : ''}"><span class="t"><b>${L('有新提交时自动跟上', 'Catch up on new commits')}</b><small>${L('仓库里有了新提交，就在后台增量分析一次。每个项目可以在项目设置里单独改。', 'Analyze new commits in the background. Each project can override this in its settings.')}</small></span><button role="switch" aria-checked="${autoDefault()}" aria-label="${L('有新提交时自动跟上', 'Catch up on new commits')}" data-fresh><i></i></button></div>`;
    sec.onclick = async (e) => {
      const t = e.target as HTMLElement;
      const lb = t.closest<HTMLElement>('[data-lang]'); if (lb) return void setLang(lb.dataset.lang as 'zh' | 'en');
      if (t.closest('[data-fresh]')) {
        store.set('autofresh', autoDefault() ? 'off' : 'on');
        void settings('general');
        if (autoFresh(state.current)) void refreshIfChanged();
        return;
      }
      const lg = t.closest<HTMLElement>('[data-logo]');
      if (lg) {
        try { await api('set_logo', { id: lg.dataset.logo }); } catch (err) { return toast(errText(err), 5000); }
        store.set('logo', lg.dataset.logo!);
        sec.querySelectorAll<HTMLElement>('[data-logo]').forEach((b) => b.setAttribute('aria-checked', String(b.dataset.logo === lg.dataset.logo)));
      }
    };
  } else if (tab === 'export') {
    sec.innerHTML = `<h3>${L('导出', 'Export')}</h3><div class="row kv" id="xrow">${native ? '' : `<span class="k">${L('保存到', 'Save to')}</span><span class="v grow">${L('浏览器的下载文件夹', "The browser's downloads")}</span>`}</div>
      <div class="row kv"><span class="k">${L('写其他贡献者的名字', 'Show contributor names')}</span><span class="v grow"></span><button class="switch" role="switch" aria-checked="${namesOn()}" aria-label="${L('写其他贡献者的名字', 'Show contributor names')}" data-x="names"><i></i></button></div>
      <p class="note">${L('格式和尺寸在每张作品的「导出」里挑，App 会记住上次用的。也可以把作品拖到下面的导出坞上。', 'Pick format and size from each card’s Export menu; the app remembers your last choice. You can also drag a card onto the export dock.')}</p>`;
    const xrow = (x: any) => {
      const r = sec.querySelector('#xrow'); if (!r || !x) return;
      r.innerHTML = `<span class="k">${L('保存到', 'Save to')}</span><span class="v path" title="${esc(x.exportDir)}">${esc(short(x.exportDir))}</span><button class="mini" data-x="choose">${L('更改…', 'Change…')}</button>${x.exportDefault ? '' : `<button class="mini" data-x="reset">${L('恢复默认', 'Reset')}</button>`}<button class="mini" data-x="open-exports">${L('打开', 'Open')}</button>`;
    };
    if (native) void api('app_info').then((x) => { if (live()) xrow(x); }).catch(() => null);
    sec.onclick = async (e) => {
      const x = (e.target as HTMLElement).closest<HTMLElement>('[data-x]')?.dataset.x;
      try {
        if (x === 'choose') { if (await api('choose_export_dir')) xrow(await api('app_info')); }
        else if (x === 'reset') { await api('reset_export_dir'); xrow(await api('app_info')); }
        else if (x === 'open-exports') await api('open_folder', { which: 'exports' });
        else if (x === 'names') {
          store.set('names', namesOn() ? 'off' : 'on');
          setNamed(namesOn());
          sec.querySelector('[data-x="names"]')?.setAttribute('aria-checked', String(namesOn()));
          state.arts.clear(); roamReset();
          if (state.raw) { rebuildModel(); await paint(); }
        }
      } catch (err) { toast(errText(err), 5000); }
    };
  } else if (tab === 'data') {
    sec.innerHTML = `<h3>${L('本地项目库', 'Library')}</h3><div id="lrow"></div><div class="row kv" id="crow" hidden></div>
      <h3>${L('AI 工具', 'AI tools')}</h3><div id="doc"><p>${L('检查中…', 'Checking…')}</p></div>`;
    const ago = (t: number) => {
      const m = Math.floor((Date.now() / 1000 - t) / 60), h = Math.floor(m / 60), d = Math.floor(h / 24);
      return m < 1 ? L('刚刚', 'just now') : m < 60 ? L(`${m} 分钟前`, `${m} min ago`) : h < 24 ? L(`${h} 小时前`, `${h} h ago`) : L(`${d} 天前`, `${d} d ago`);
    };
    const crow = (c: any) => {
      const r = sec.querySelector<HTMLElement>('#crow'); if (!r || !c) return;
      r.hidden = false;
      // 定时把新记录同步至本地项目库。三档：不备份 / App 开着时 / 一直（macOS 的后台任务）
      const mode = c.agent?.on ? 'always' : c.on ? 'open' : 'off';
      const modes: [string, string][] = [['off', L('关', 'Off')], ['open', L('App 开着时', 'While open')], ...(c.agent?.supported ? [['always', L('一直', 'Always')]] as [string, string][] : [])];
      const st = c.running ? L('正在备份…', 'Backing up…') : mode !== 'off' && c.last?.at ? L(`上次 ${ago(c.last.at)}`, `Last ${ago(c.last.at)}`) : '';
      r.innerHTML = `<span class="k" title="${L('将本地编程工具记录同步至本地存档，防止历史记录遗失', 'Sync local tool logs to the local project archive')}">${L('备份 AI 工具记录', 'Back up AI tool logs')}</span><span class="v grow">${esc(st)}</span>`
        + `<div class="seg" role="radiogroup">${modes.map(([m, n]) => `<button data-x="collect-mode" data-m="${m}" role="radio" aria-checked="${m === mode}" class="${m === mode ? 'on' : ''}">${esc(n)}</button>`).join('')}</div>`;
      // 一轮正在跑（定时的那轮）：跑完换成结果
      if (c.running) setTimeout(() => { if (live()) api('collect_info').then(crow).catch(() => null); }, 2000);
    };
    if (native) void api('app_info').then((info) => {
      const r = sec.querySelector('#lrow'); if (!r || !live()) return;
      r.innerHTML = `<div class="row kv"><span class="k">${L('位置', 'Location')}</span><span class="v path" title="${esc(info.library)}">${esc(short(info.library))}</span><button class="mini" data-x="open-library">${L('打开', 'Open')}</button></div>`;
    }).catch(() => null);
    void api('collect_info').then((c) => { if (live()) crow(c); }).catch(() => null);
    sec.onclick = async (e) => {
      const t = e.target as HTMLElement, x = t.closest<HTMLElement>('[data-x]')?.dataset.x;
      try {
        if (x === 'open-library') await api('open_folder', { which: 'library' });
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
    let doc: any[];
    try { doc = await api('doctor'); }
    catch (e) {
      const box = sec.querySelector('#doc'); if (!box || !live()) return;
      box.innerHTML = `<p>${esc(L('检查数据源失败', 'Could not check data sources'))}：${esc(errText(e))}</p><button class="mini">${L('重试', 'Retry')}</button>`;
      box.querySelector('button')!.onclick = () => void settings('data');
      return;
    }
    const detail = (d: any) => d.scope === 'sessions' && d.ok ? L('仅会话信息', 'Session information only')
      : d.scope === 'capture' && !d.ok ? L('需开启采集', 'Capture required')
      : d.unsupported ? L('编辑格式待接', 'Edit format unconfirmed')
      : !d.ok ? L('没有找到', 'Not found')
      : d.records != null ? L(`${fmt(d.records)} 条记录`, `${fmt(d.records)} records`)
      : d.entries != null ? L(`${fmt(d.entries)} 个条目`, `${fmt(d.entries)} entries`)
      : L('已就绪', 'Ready');
    const box = sec.querySelector('#doc'); if (!box || !live()) return;
    box.innerHTML = doc.length ? doc.map((d: any) => `<div class="row" title="${esc(d.path ?? '')}"><i class="dot ${d.ok && !d.unsupported ? 'ok' : ''}"></i><span class="k">${esc(sourceName(d.id))}</span><span class="v">${esc(detail(d))}</span></div>`).join('') : `<p>${L('没有可用的数据源', 'No data sources found')}</p>`;
  } else {
    sec.innerHTML = `<div class="aboutbox"><span class="am">${markEl(BRAND)}</span><b>creditia</b><small id="ver"></small>
      <p>${L('看清每个 AI 工具在你的仓库里写了多少，再把它做成能放进 README、发版说明和作品集的署名。', 'See how much each AI tool wrote in your repos, then turn it into credits for your README, releases and portfolio.')}</p>
      <p class="about">${L('只在本机运行，不上传任何代码或记录。', 'Runs only on this computer. No code or logs are uploaded.')}</p></div>`;
    if (native) void api('app_info').then((info) => { const v = sec.querySelector('#ver'); if (v && live()) v.textContent = `${L('版本', 'Version')} ${info.version}`; }).catch(() => null);
  }
}

boot();
