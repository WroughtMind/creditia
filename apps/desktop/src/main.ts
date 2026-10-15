// creditia App：一面展墙。顶部切换项目，左下角圆点切换风格，所有作品直接摆出来；
// 悬停看细节、换版式、选格式导出；拖到右下角的导出坞也能导出。
import { buildModel, type Model } from '../../../packages/render/src/classic/data.ts';
import { nameOf, resolveGroup } from '../../../packages/render/src/data.ts';
import { MODEL_VARIANTS } from '../../../packages/render/src/registry.ts';
import { mergeCredits } from '../../../packages/render/src/merge.ts';
import { creditsBlock, mergeCreditsMd } from '../../../packages/render/src/classic/text.ts';
import { FORMATS, SIZES, type Art } from '../../../packages/render/src/export.ts';
import { INTERACT_CSS, mountHits } from '../../../packages/render/src/interact.ts';
import { KITS, CLASSIC_COUNT, kitById, bootKits, setGroup, groupOf, hasModels, type Kit } from './kits.ts';
import { exportArt, type Format } from './engine.ts';
import { LOGO_SVG } from './logo.ts';

// ───────────────────────── 桥：Tauri 或浏览器调试 ─────────────────────────
const native = (window as any).__TAURI__?.core?.invoke as ((cmd: string, args?: any) => Promise<any>) | undefined;
/** 官网上的演示：页面先放好样例档案（window.__CREDITIA_DEMO__），导出的文件交给外层页面摆到「桌面」上 */
const demo = (window as any).__CREDITIA_DEMO__ as { projects: any[]; credits: Record<string, any>; doctor: any[] } | undefined;
const devPlugs: Plugs = {
  hook: { installed: false, decided: null, path: '.git/hooks/prepare-commit-msg' },
  release: { installed: false, decided: null, path: '.github/workflows/creditia.yml', github: true, published: true },
};
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
    // 浏览器里（开发、官网演示）没有仓库可写：开关只在内存里拨
    case 'integrations': return structuredClone(devPlugs);
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
  switch_failed: ['没能改这一项', "Couldn't change this"],
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
  applyLang(); renderDots(); renderDock(); wireTop(); wireRoam();
  // 滚动之后顶栏垫一层渐隐的玻璃，作品不再直接透到项目名和按钮后面
  const stageEl = $('#stage');
  stageEl.addEventListener('scroll', () => document.body.classList.toggle('scrolled', stageEl.scrollTop > 8), { passive: true });
  try { state.projects = await api('projects'); } catch (e: any) { return empty(() => [L('读不到作品库', 'Cannot read the library'), errText(e)]); }
  if (!state.projects.length) return empty(() => [L('还没有分析过的项目', 'No analyzed projects yet'), '', 'creditia analyze', native ? { label: L('添加项目…', 'Add a project…'), run: addProject } : undefined]);
  const last = (store.get('project') === ALL_ID && state.projects.length > 1 ? allProject() : null) ?? state.projects.find((p) => p.id === store.get('project')) ?? state.projects[0];
  await openProject(last);
  addEventListener('resize', () => { layout(); if (roam.ready) roamReset(); });
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
  el.innerHTML = `<div><b>${esc(title)}</b>${esc(detail)}${act ? `<br><button class="go glass">${esc(act.label)}</button>` : ''}${cmd ? `<br><span>${act ? L('或者在项目目录里运行', 'or run in your project') : L('在项目目录里运行', 'Run in your project')}</span><br><code>${esc(cmd)}</code>` : ''}</div>`;
  if (act) (el.querySelector('.go') as HTMLElement).onclick = act.run;
  if (!keepName) { $('#pname').textContent = 'creditia'; $('#pmeta').textContent = ''; }
}

async function openProject(p: any) {
  const seq = ++state.openSeq;
  state.current = p; store.set('project', p.id);
  state.raw = null; state.model = null; state.arts.clear(); state.variants = {};
  $('#group').hidden = true; plugs = null; renderPlugs();
  ++state.token; // 让正在画的旧项目停下
  $('#board').innerHTML = '';
  $('#pname').textContent = p.name; $('#pmeta').textContent = L('读取中…', 'Loading…');
  let raw: any;
  const st = readStamp(p); // 先记下版本再读档案：读的过程中又变了，下次比对还能发现
  try { raw = await creditsOf(p); }
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
    toast(L('档案有更新，已换成最新的', 'Credits changed — showing the latest'));
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
  if (state.current?.all && state.raw?.project) state.raw.project.name = L('全部项目', 'All projects');
  state.model = buildModel(state.raw, { director: false, lang: state.lang, group: groupPref() });
  const m = state.model;
  $('#pname').textContent = m.project || state.current?.name;
  $('#pmeta').textContent = `${fmt(m.commits)} ${L('次提交', 'commits')} · ${fmt(m.linesAdded)} ${L('行', 'lines')}`;
  renderGroup();
  renderPlugs();
}

// ───────────────────────── 接入：提交盖章、跟着发版 ─────────────────────────
// 两项都会改用户的仓库，所以每个项目由用户自己决定。没决定过时这一条醒目地摆在作品墙最上面问；
// 决定过之后照样留在那里，随时能拨回去。开关交给核心的 hook / release install|uninstall，决定记进项目配置
type Plug = { installed: boolean; decided: boolean | null; path: string; github?: boolean; published?: boolean };
type Plugs = { hook: Plug; release: Plug };
let plugs: Plugs | null = null, plugBusy = '';
async function loadPlugs(p: any) {
  try { const r = await api('integrations', { projectId: p.id }); if (p === state.current) plugs = r; }
  catch { if (p === state.current) plugs = null; }
  renderPlugs();
}
function renderPlugs() {
  const el = $('#plug');
  if (!plugs || !state.raw) { el.hidden = true; return; }
  const ask = (['hook', 'release'] as const).filter((k) => plugs![k].decided == null);
  el.hidden = false;
  el.classList.toggle('ask', ask.length > 0);
  el.classList.toggle('done', ask.length === 0);
  const rel = plugs.release;
  const item = (k: 'hook' | 'release', title: string, desc: string, note = '') => {
    const on = plugs![k].installed;
    // 决定过之后收成一行：说明放进悬停提示，只有提醒（note）还摆出来
    return `<div class="sw${on ? ' on' : ''}${plugBusy === k ? ' busy' : ''}${plugs![k].decided == null ? ' new' : ''}"${ask.length ? '' : ` title="${esc(desc)}"`}>
      <span class="t"><b>${esc(title)}</b><small>${esc(desc)}</small>${note ? `<small class="note">${esc(note)}</small>` : ''}</span>
      <button role="switch" aria-checked="${on}" aria-label="${esc(title)}" data-k="${k}" ${plugBusy ? 'disabled' : ''}><i></i></button></div>`;
  };
  el.innerHTML = (ask.length ? `<p class="q"><b>${L('这个项目要接入下面两项吗？', 'Add these to this project?')}</b>${L('都只改这一个仓库，随时能关。', 'Each changes only this repository and can be turned off any time.')}</p>` : '')
    + `<div class="row">${ask.length ? '' : `<span class="lab">${L('接入', 'Integrations')}</span>`}${item('hook', L('提交盖章', 'Commit stamps'), L('提交信息自动带上工具、模型和各工具写的行数。日志清理后也认得出，PR 评论读它', 'Commits carry the tool, model and lines per tool, so they stay recognizable after logs are cleaned up; PR comments read them'))}`
    + item('release', L('跟着发版', 'Release credits'), L('发 Release 时把署名写进说明，PR 上评论署名。写一个 GitHub workflow，要你提交', 'Credits go into release notes and PR comments. Writes a GitHub workflow for you to commit'),
      rel.installed && !rel.published && !plugs.hook.installed ? L('它读提交盖章记下的行数：把左边的提交盖章也打开', 'It reads the line counts from commit stamps: turn those on too') : rel.github === false && !rel.installed ? L('这个仓库没有 GitHub 远程', 'This repository has no GitHub remote') : '')
    + `</div>` + (ask.length ? `<button class="later" data-act="later">${L(ask.length > 1 ? '都先不用' : '先不用', ask.length > 1 ? 'Not now, neither' : 'Not now')}</button>` : '');
  el.querySelectorAll<HTMLElement>('button[data-k]').forEach((b) => (b.onclick = () => setPlug(b.dataset.k as 'hook' | 'release', !plugs![b.dataset.k as 'hook' | 'release'].installed)));
  const later = el.querySelector<HTMLElement>('.later');
  if (later) later.onclick = async () => { for (const k of ask) await setPlug(k, false, true); };
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
// 默认「自动」：一个工具占了绝大部分、下面又有几个模型时自动按模型分。点一下就钉住这个项目的选择，
// 「↺ 自动」随时切回去。档案里一个模型都没记下就不显示。
type GroupPref = 'auto' | 'tool' | 'model';
const groupPref = (): GroupPref => { const g = state.current ? store.get('group.' + state.current.id) : null; return g === 'tool' || g === 'model' ? g : 'auto'; };
function renderGroup() {
  const el = $('#group');
  if (!state.raw || !hasModels(state.raw)) { el.hidden = true; return; }
  const pref = groupPref(), now = groupOf(state.raw);
  const auto = pref === 'auto';
  el.hidden = false;
  el.setAttribute('aria-label', L('分组方式', 'Group by'));
  el.innerHTML = (['tool', 'model'] as const).map((g) => `<button data-g="${g}" class="${g === now ? 'on' : ''}" aria-pressed="${g === now}">${g === 'tool' ? L('按工具', 'By tool') : L('按模型', 'By model')}</button>`).join('')
    + (auto ? `<span class="auto" title="${L('按档案自动选的，点左边可以改', 'Picked automatically from the credits — click to change')}">${L('自动', 'Auto')}</span>`
            : `<button data-g="auto" class="back" title="${L('交给 creditia 自动选', 'Let creditia pick')}">↺ ${L('自动', 'Auto')}</button>`);
  el.querySelectorAll<HTMLElement>('button').forEach((b) => (b.onclick = () => pickGroup(b.dataset.g as GroupPref)));
}
async function pickGroup(g: GroupPref) {
  const p = state.current; if (!p || !state.raw) return;
  if (g === groupPref() && g !== 'auto') return;
  try { if (g === 'auto') localStorage.removeItem('creditia.group.' + p.id); else store.set('group.' + p.id, g); } catch { /* 没有存储也能用 */ }
  setGroup(g);
  rebuildModel();
  await paint();
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
  const key = `${state.style.id}|${state.lang}|${groupPref()}|${id}|${variant ?? ''}`;
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
  applyView(); if (roam.ready) roamReset();
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
    tile.dataset.span = String(b.span ?? (a.w / a.h > 2.2 ? 2 : ''));
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

/** 瀑布流：一栏宽度跟着窗口变，横向作品跨两栏，小件不放大。
 *  跨栏的作品会在矮的那栏下面留洞，所以每次往后看几张，挑放下去最高、留洞最少的那张先放 */
function layout() {
  const board = $('#board');
  const tiles = [...board.querySelectorAll('.tile')] as HTMLElement[];
  const W = Math.min(innerWidth - 120, 1680);
  const gap = 40;
  const cols = Math.max(1, Math.min(4, Math.floor((W + gap) / (400 + gap))));
  const cw = (W - gap * (cols - 1)) / cols;
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
    const small = Number(t.dataset.w) < 500; // 徽章这种小件，跨栏也只是居中，有洞时放一栏
    if (set && !(small && set > 1)) return at(t, Math.min(cols, set));
    // 很扁的图表在窄栏里字太小，三栏以上时让它跨两栏；跨了会留大洞就还是放一栏
    const wide = small ? set > 1 : cols >= 3 && Number(t.dataset.w) / Number(t.dataset.h) > 1.7;
    const two = wide ? at(t, 2) : null;
    return two && two.hole < 160 && two.y - Math.min(...hs) < 160 ? two : at(t, 1);
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
  fresh.className = 'tile placed'; fresh.dataset.k = k; fresh.dataset.w = String(a.w); fresh.dataset.h = String(a.h); fresh.style.left = tile.style.left; fresh.style.top = tile.style.top; fresh.style.width = tile.style.width; fresh.dataset.span = String(b.span ?? (a.w / a.h > 2.2 ? 2 : '')); fresh.style.animation = 'none';
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
  const base = `${m.project || 'credits'}-${id}${variant && variant !== 'original' ? '-' + variant : ''}-${state.style.id}`;
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
    // 一个工具里有几个模型：分层画工具和模型的版式加个小点
    const multiModel = !!state.raw && resolveGroup(state.raw, 'auto') === 'model';
    el.innerHTML = `${closeBtn()}
      <div class="big"><div class="art"></div></div>
      <div class="strip glass">
        <b style="padding:0 8px">${esc(state.style.pieceName(id, state.lang))}</b><span style="color:var(--soft)">${esc(state.style.pieceQ(id, state.lang))}</span><span class="keys" title="${L('上一件 / 下一件作品', 'Previous / next piece')}"><kbd>↑</kbd><kbd>↓</kbd></span>
        ${vs.length > 1 ? `<span class="sep"></span><span class="lab">${L('版式', 'Layout')}</span><span class="keys" title="${L('上一种 / 下一种版式', 'Previous / next layout')}"><kbd>←</kbd><kbd>→</kbd></span>${vs.map((v) => { const mm = multiModel && MODEL_VARIANTS[id]?.includes(v); return `<button data-v="${v}" class="${v === variant ? 'on' : ''}${mm ? ' mm' : ''}"${mm ? ` title="${L('工具和它的各个模型分层画出来', 'Draws each tool with its models underneath')}"` : ''}>${esc(state.style.variantName(v, state.lang))}</button>`; }).join('')}` : ''}
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

// ───────────────────────── 漫游：上下左右没有尽头的画布 ─────────────────────────
// 墙之外的另一种看法。每一栏自己往上往下无限延伸，栏和栏也无限往左右排；作品按顺序轮着出现，
// 每轮一圈就换下一种版式，所以走得越远，看到的是同一件作品的别的画法，而不是原样复制。
// 拖动（或滚轮、触控板）移动画布；点一下放大，悬停点「导出」。只把视口附近的作品放进页面。
type RItem = { r: number; y: number; h: number; w: number; id: string; k: string; v?: string; a: Art };
type RCol = { items: RItem[]; busy: boolean };
const roam = { x: 0, y: 0, cols: new Map<number, RCol>(), els: new Map<string, HTMLElement>(), gen: 0, vx: 0, vy: 0, raf: 0, ready: false };
const CAP = 30, RGAP = 56, STEP = 5;
const view = () => (store.get('view') === 'roam' ? 'roam' : 'wall');
const rcw = () => Math.round(Math.max(380, Math.min(520, innerWidth / 3.1)));
const pitch = () => rcw() + RGAP;
const mod = (a: number, n: number) => ((a % n) + n) % n;
/** 每一栏起点错开一点，栏和栏的作品不会排成一行一行 */
const colOff = (X: number) => mod(Math.imul(X ^ 0x5bd1e995, 0x27d4eb2d) >>> 0, 997) / 997 * 420;

/** 第 X 栏第 r 个是哪件作品、哪个版式 */
function slot(X: number, r: number) {
  const list = tiles(), n = list.length;
  const v = X + r * STEP, i = mod(v, n), lap = Math.floor(v / n);
  const b = list[i], k = keyOf(b, i);
  const vs = state.style.variants(b.id);
  const base = curVariant(b.id, k, b);
  const j = vs.indexOf(base ?? '');
  return { b, k, v: vs.length > 1 ? vs[mod((j < 0 ? 0 : j) + lap, vs.length)] : base };
}
async function rItem(X: number, r: number): Promise<RItem | null> {
  const s = slot(X, r);
  let a: Art | null = null;
  try { a = await drawArt(s.b.id, s.v); } catch (e) { console.error(s.b.id, e); }
  if (!a) return null;
  const cw = rcw(), w = Math.min(cw, a.w < 500 ? a.w * 1.15 : cw);
  return { r, y: 0, w, h: (w / a.w) * a.h, id: s.b.id, k: s.k, v: s.v, a };
}
/** 让第 X 栏盖住 [top, bottom]；一栏同时只长一处，长完再看一遍 */
async function grow(X: number, top: number, bottom: number) {
  let col = roam.cols.get(X);
  if (!col) { col = { items: [], busy: false }; roam.cols.set(X, col); }
  if (col.busy) return;
  col.busy = true;
  const gen = roam.gen;
  try {
    let guard = 0;
    while (gen === roam.gen && guard++ < 40) {
      const it = col.items;
      if (!it.length) {
        const f = await rItem(X, 0); if (gen !== roam.gen) return;
        if (!f) break;
        f.y = colOff(X); it.push(f); continue;
      }
      const last = it[it.length - 1], first = it[0];
      if (last.y + last.h < bottom) {
        const f = await rItem(X, last.r + 1); if (gen !== roam.gen) return;
        if (!f) { last.r++; continue; } // 画不出来的跳过
        f.y = last.y + last.h + RGAP + CAP; it.push(f);
      } else if (first.y - CAP > top) {
        const f = await rItem(X, first.r - 1); if (gen !== roam.gen) return;
        if (!f) { first.r--; continue; }
        f.y = first.y - CAP - RGAP - f.h; it.unshift(f);
      } else break;
      place();
    }
  } finally { col.busy = false; }
  if (gen === roam.gen) place();
}
/** 只把视口附近的作品放进页面，远的拿掉；缺的让那一栏去长 */
function place() {
  if (!roam.ready) return;
  const plane = $('#roam .plane');
  plane.style.transform = `translate3d(${-roam.x}px, ${-roam.y}px, 0)`;
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
      el.innerHTML = `<div class="name">${esc(state.style.pieceName(t.id, state.lang))}${t.v && state.style.variants(t.id).length > 1 ? `<span class="vn">${esc(state.style.variantName(t.v, state.lang))}</span>` : ''}</div>
        <div class="bar glass"><button data-act="export" title="${L('导出', 'Export')}">${L('导出', 'Export')} ↓</button></div><div class="art"></div>`;
      const artEl = el.querySelector('.art') as HTMLElement;
      artEl.innerHTML = t.a.svg;
      mountHits(artEl, t.a.hits, t.a.w, t.a.h);
      (el as any)._item = t;
      plane.appendChild(el);
      roam.els.set(key, el);
    }
  }
  for (const [key, el] of roam.els) if (!want.has(key)) { el.remove(); roam.els.delete(key); }
}
/** 墙上那件作品借用这一张的版式和画好的图：放大、导出都照这一张来 */
function adopt(t: RItem) { state.variants[t.k] = t.v ?? ''; state.arts.set(t.k, t.a); }
/** 回到起点：第一栏贴着左边居中排开，最上面让出顶栏 */
function home(): [number, number] {
  const P = pitch(), n = Math.max(1, Math.floor((innerWidth + RGAP) / P));
  return [-Math.round((innerWidth - (n * P - RGAP)) / 2), -96];
}
function roamReset() {
  roam.gen++;
  roam.cols.clear(); roam.els.clear();
  $('#roam .plane').innerHTML = '';
  if (view() === 'roam' && state.model) place();
}
function glide(tx: number, ty: number) {
  cancelAnimationFrame(roam.raf);
  const x0 = roam.x, y0 = roam.y, t0 = performance.now(), d = 520;
  const step = (now: number) => {
    const p = Math.min(1, (now - t0) / d), e = 1 - Math.pow(1 - p, 3);
    roam.x = x0 + (tx - x0) * e; roam.y = y0 + (ty - y0) * e; place();
    if (p < 1) roam.raf = requestAnimationFrame(step);
  };
  roam.raf = requestAnimationFrame(step);
}
function wireRoam() {
  const el = $('#roam');
  let drag: { x: number; y: number; px: number; py: number; moved: boolean; t: number; hist: [number, number, number][] } | null = null;
  el.addEventListener('pointerdown', (e) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest('button, .fmts')) return;
    cancelAnimationFrame(roam.raf);
    drag = { x: e.clientX, y: e.clientY, px: roam.x, py: roam.y, moved: false, t: performance.now(), hist: [] };
    el.setPointerCapture(e.pointerId);
  });
  el.addEventListener('pointermove', (e) => {
    if (!drag) return;
    const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
    if (!drag.moved && Math.hypot(dx, dy) > 5) { drag.moved = true; el.classList.add('panning'); closeFormats(); }
    if (!drag.moved) return;
    roam.x = drag.px - dx; roam.y = drag.py - dy;
    drag.hist.push([performance.now(), roam.x, roam.y]); if (drag.hist.length > 6) drag.hist.shift();
    place();
  });
  const up = (e: PointerEvent) => {
    if (!drag) return;
    const d = drag; drag = null;
    el.classList.remove('panning');
    if (!d.moved) {
      if (e.type !== 'pointerup') return;
      // 没拖动就是点了一下：放大这一张
      const tile = document.elementsFromPoint(e.clientX, e.clientY).find((x) => x.classList.contains('rt')) as HTMLElement | undefined;
      const t = (tile as any)?._item as RItem | undefined;
      if (t) { adopt(t); openFocus(t.id, t.k); }
      return;
    }
    // 松手后按最后一小段的速度滑一会儿
    const h = d.hist; if (h.length < 2) return;
    const [t1, x1, y1] = h[0], [t2, x2, y2] = h[h.length - 1], dt = Math.max(16, t2 - t1);
    if (performance.now() - t2 > 80) return;
    let vx = ((x2 - x1) / dt) * 16, vy = ((y2 - y1) / dt) * 16;
    const coast = () => {
      vx *= 0.94; vy *= 0.94;
      if (Math.hypot(vx, vy) < 0.3) return;
      roam.x += vx; roam.y += vy; place();
      roam.raf = requestAnimationFrame(coast);
    };
    roam.raf = requestAnimationFrame(coast);
  };
  el.addEventListener('pointerup', up); el.addEventListener('pointercancel', up);
  el.addEventListener('wheel', (e) => {
    e.preventDefault(); cancelAnimationFrame(roam.raf);
    const k = e.deltaMode === 1 ? 16 : 1;
    // 普通鼠标滚轮只有上下：按住 Shift 就是左右
    if (e.shiftKey && !e.deltaX) roam.x += e.deltaY * k; else { roam.x += e.deltaX * k; roam.y += e.deltaY * k; }
    place();
  }, { passive: false });
  el.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest('button[data-act="export"]'); if (!b) return;
    e.stopPropagation();
    const tile = b.closest('.rt') as HTMLElement, t = (tile as any)._item as RItem;
    adopt(t); toggleFormats(tile, t.id, t.k);
  });
  $('#home').onclick = () => glide(...home());
  $('#view').onclick = (e) => {
    const b = (e.target as HTMLElement).closest('button') as HTMLElement | null; if (!b) return;
    store.set('view', b.dataset.v!); applyView();
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
  $('#view').hidden = !state.model;
  const was = document.body.classList.contains('roam');
  document.body.classList.toggle('roam', on);
  $('#roam').hidden = !on; $('#home').hidden = !on;
  if (on && !roam.ready) { roam.ready = true; [roam.x, roam.y] = home(); roamReset(); }
  else if (on && !was) place();
}

// ───────────────────────── 风格圆点 ─────────────────────────
function renderDots() {
  const nav = $('#dots');
  // 原版主题和新风格之间留一道细线
  nav.innerHTML = `<span class="cur" id="curstyle">${esc(state.style.name(state.lang))}</span>` + KITS.map((t, i) => `${i === CLASSIC_COUNT ? '<i class="divider"></i>' : ''}<button data-s="${t.id}" class="${t.id === state.style.id ? 'on' : ''}" aria-label="${esc(t.name(state.lang))}"><svg viewBox="0 0 24 24">${t.dot}</svg></button>`).join('');
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
    $('#curstyle').textContent = t.name(state.lang);
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
  const [w, r] = document.querySelectorAll<HTMLElement>('#view button');
  w.textContent = L('墙', 'Wall'); r.textContent = L('漫游', 'Roam');
  w.title = L('整理好的一面墙', 'Everything on one tidy wall'); r.title = L('没有尽头的画布：拖动、滚动随便走，越远看到越多版式', 'An endless canvas: drag or scroll anywhere; the farther you go, the more layouts you see');
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
function closeMenu() { $('#menu').hidden = true; $('#project').setAttribute('aria-expanded', 'false'); }
function toggleMenu() {
  const menu = $('#menu'), pill = $('#project');
  if (!menu.hidden) return closeMenu();
  pill.setAttribute('aria-expanded', 'true');
  menu.hidden = false;
  menu.innerHTML = `<input id="q" placeholder="${L('搜索项目', 'Search projects')}" autocomplete="off"><div class="list" id="plist" role="listbox" aria-label="${L('项目', 'Projects')}"></div>
    <div class="foot"><button id="add">${L('＋ 添加项目', '+ Add project')}</button><button id="re">↻ ${L('重新分析', 'Re-analyze')}</button></div>
    <div class="foot"><button id="auto" role="switch" aria-checked="${autoFresh()}" title="${L('有新提交时在后台增量分析，作品跟着更新', 'Analyze new commits in the background so pieces stay current')}">${autoFresh() ? '●' : '○'} ${L('自动跟上新提交', 'Keep up with new commits')}</button></div>`;
  const draw = () => {
    const q = ($('#q') as HTMLInputElement).value.trim().toLowerCase();
    const all = state.projects.length > 1 && (!q || L('全部项目', 'All projects').toLowerCase().includes(q) || 'all'.includes(q))
      ? `<div class="item all ${state.current?.all ? 'on' : ''}" data-id="${ALL_ID}" role="option" aria-selected="${!!state.current?.all}"><b>${L('全部项目', 'All projects')}</b><span class="r"></span><span>${L(`${state.projects.length} 个项目合在一起`, `${state.projects.length} projects together`)}</span><span class="r"></span></div>` : '';
    $('#plist').innerHTML = all + state.projects.filter((p) => !q || `${p.name} ${p.path}`.toLowerCase().includes(q)).map((p) => `
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
    if (it.dataset.id === ALL_ID) { if (!state.current?.all) openProject(allProject()); return; }
    const p = state.projects.find((x) => x.id === it.dataset.id);
    if (p && p !== state.current) openProject(p);
  };
  const re = $('#re') as HTMLButtonElement;
  if (!state.current || state.current.all || state.current.exists === false || analyzing) { re.disabled = true; re.title = state.current?.exists === false ? L('项目目录已经不在了', 'The project folder is gone') : ''; }
  ($('#add') as HTMLButtonElement).disabled = analyzing;
  $('#add').onclick = () => { closeMenu(); addProject(); };
  re.onclick = () => { closeMenu(); if (state.current) reanalyze(state.current); };
  $('#auto').onclick = () => {
    store.set('autofresh', autoFresh() ? 'off' : 'on');
    const b = $('#auto'); b.setAttribute('aria-checked', String(autoFresh())); b.textContent = `${autoFresh() ? '●' : '○'} ${L('自动跟上新提交', 'Keep up with new commits')}`;
    if (autoFresh()) refreshIfChanged();
  };
  setTimeout(() => document.addEventListener('click', function off(e) { if (!menu.contains(e.target as Node) && !pill.contains(e.target as Node)) { menu.hidden = true; pill.setAttribute('aria-expanded', 'false'); document.removeEventListener('click', off); } }), 0);
}

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
    if (auto) toast(n ? L(`已跟上 ${n} 个新提交`, `Caught up with ${n} new commit${n > 1 ? 's' : ''}`) : L('已更新', 'Updated'));
    else done(L('已更新', 'Updated'), v?.warnings);
  } catch (e) { if (!auto || (e as any)?.code !== 'busy') toast(errText(e), 6000); }
}
async function addProject() {
  let dir: string | null;
  try { dir = await api('pick_folder'); } catch (e) { return toast(errText(e), 5000); }
  if (!dir) return;
  const name = dir.split(/[\\/]/).filter(Boolean).pop() ?? dir;
  try {
    const r = await withProgress(name, () => api('add_project', { path: dir, lang: state.lang }));
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
  /** doctor 的数据源名：一个工具有两处记录时分开写 */
  const SOURCES: Record<string, [string, string]> = { 'copilot-vscode': ['Copilot（VS Code）', 'Copilot (VS Code)'], 'copilot-cli': ['Copilot 命令行', 'Copilot CLI'], 'trae-cli': ['TRAE 命令行', 'TRAE CLI'], 'trae-ide': ['TRAE IDE', 'TRAE IDE'], 'kilo-cli': ['Kilo 命令行', 'Kilo CLI'], antigravity: ['Antigravity 命令行', 'Antigravity CLI'], windsurf: ['Windsurf', 'Windsurf'] };
  const sourceName = (id: string) => (SOURCES[id] ? L(...SOURCES[id]) : nameOf(id, state.lang));
  const doc = await api('doctor').catch(() => []);
  const detail = (d: any) => d.unsupported ? L('读不了编辑记录', 'Edits not readable')
    : !d.ok ? L('没有找到', 'Not found')
    : d.records != null ? L(`${fmt(d.records)} 条记录`, `${fmt(d.records)} records`)
    : d.entries != null ? L(`${fmt(d.entries)} 个条目`, `${fmt(d.entries)} entries`)
    : L('已就绪', 'Ready');
  $('#doc').innerHTML = doc.length ? doc.map((d: any) => `<div class="row" title="${esc(d.path ?? '')}"><i class="dot ${d.ok && !d.unsupported ? 'ok' : ''}"></i><span class="k">${esc(sourceName(d.id))}</span><span class="v">${esc(detail(d))}</span></div>`).join('') : `<p>${L('没有可用的数据源', 'No data sources found')}</p>`;
}

boot();
