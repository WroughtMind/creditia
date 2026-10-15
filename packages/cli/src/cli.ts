// creditia CLI：分析交给 Rust 核心（透传），这里负责档案、导出、终端名片。
// 所有命令非交互、支持 --json，方便 agent 代人调用。
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { deflateSync } from 'node:zlib';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { art, buildModel, FORMS, themeById, raster, type Model, type Theme } from '../../render/src/classic/index.ts';
import { ORDERED_THEMES, themeDot, themeName, themeNote, pieceName, pieceQ, variantName } from '../../render/src/classic/catalog.ts';
import { makeExporter, sized, SIZES, FORMATS, type Art } from '../../render/src/export.ts';
import { creditsBlock, mergeCreditsMd, releaseNotes, prComment, fetchSegs } from '../../render/src/text.ts';
import { BRAND } from '../../render/src/style.ts';
import * as V2 from '../../render/src/index.ts';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '../../..');
const deflate = (d: Uint8Array) => new Uint8Array(deflateSync(d));
// 原版作品的文字已经是路径，栅格化不需要字体；新风格的作品要带字体
const ex = makeExporter(raster, deflate);
const exV2 = makeExporter(V2.raster, deflate);

// ───────────────────────── 参数 ─────────────────────────
const SWITCHES = new Set(['json', 'fresh', 'force', 'github', 'all', 'yes', 'list', 'no-color', 'refresh']);
const argv = process.argv.slice(2);
const cmd = argv[0] ?? '';
const pos: string[] = [];
const flags: Record<string, string> = {};
const on = new Set<string>();
for (let i = 1; i < argv.length; i++) {
  const x = argv[i];
  if (x.startsWith('--')) {
    const k = x.slice(2);
    const eq = k.indexOf('=');
    if (eq > 0) flags[k.slice(0, eq)] = k.slice(eq + 1);
    else if (SWITCHES.has(k) || i + 1 >= argv.length || argv[i + 1].startsWith('--')) on.add(k);
    else flags[k] = argv[++i];
  } else pos.push(x);
}
const json = on.has('json');
/** 文案语言：--lang，其次环境变量 CREDITIA_LANG，默认中文。也传给核心 */
const LANG = (): 'zh' | 'en' => ((flags.lang ?? process.env.CREDITIA_LANG ?? 'zh').toLowerCase().startsWith('en') ? 'en' : 'zh');
const L = (zh: string, en: string) => (LANG() === 'en' ? en : zh);

const USAGE_ZH = `用法：creditia <命令> [选项]

分析（Rust 核心，全程本地，不调用任何大模型）
  discover                          列出本机有 agent 活动的项目
  init      [--repo P]              在仓库写 .creditia/config.json
  analyze   [--repo P] [--github] [--level strict|standard|loose]
  update    [--repo P]              增量更新
  collect   [--repo P | --all]      只采集证据
  doctor                            检查各数据源
  blame     <路径>:<行号>           这一行是谁写的
  paths | link <路径> | unlink <路径>   一个项目的多处路径
  whoami                            当前在哪个 agent 里运行
  stamp     --harness H [--model M] 给下一次提交盖章
  hook      install|uninstall       提交盖章 hook
  projects                          作品库里的项目

档案与展示
  doc            [--out CREDITS.md]                     生成或更新 CREDITS.md（只改标记区间）
  release-notes  <tag|unreleased>                       该版本的 credits 段落
  pr-comment     <N>                                    该 PR 的一句话归因
  export         --piece P [--style S] [--variant V] [--format png|svg|pdf|html|md|json]
                 [--size og|square|portrait|story|a4|WxH] [--lang zh|en] [--out FILE]
  export         --list                                 全部作品、版式、风格、格式
  board          [--style S] [--out credits.html]       全部作品放进一个可交互的 HTML 页面
  site           [--out 目录] [--style S]               一个小网站：首页是全部作品，每种风格一页（给 GitHub Pages）
  publish        [--out credits.json]                   把档案放到仓库根目录，准备公开（会列出将公开的内容）

任何展示命令都可以加 --credits FILE，直接用一份现成的档案（CI 里没有 agent 日志，用的就是仓库里公开的 credits.json）
  fetch                                                 终端名片

所有命令支持 --json 和 --lang zh|en（也可以设环境变量 CREDITIA_LANG）。`;

const USAGE_EN = `Usage: creditia <command> [options]

Analysis (Rust core, all local, never calls a model)
  discover                          list projects on this machine with agent activity
  init      [--repo P]              write .creditia/config.json in the repository
  analyze   [--repo P] [--github] [--level strict|standard|loose]
  update    [--repo P]              incremental update
  collect   [--repo P | --all]      only collect evidence
  doctor                            check every data source
  blame     <path>:<line>           which tool and model wrote this line
  paths | link <path> | unlink <path>   one project in several places
  whoami                            which agent this is running in
  stamp     --harness H [--model M] stamp the next commit
  hook      install|uninstall       the commit-stamp hook
  projects                          projects in the library

Credits and pieces
  doc            [--out CREDITS.md]                     write or update CREDITS.md (only between the markers)
  release-notes  <tag|unreleased>                       the credits paragraph for a version
  pr-comment     <N>                                    a one-line attribution for a PR
  export         --piece P [--style S] [--variant V] [--format png|svg|pdf|html|md|json]
                 [--size og|square|portrait|story|a4|WxH] [--lang zh|en] [--out FILE]
  export         --list                                 every piece, layout, style and format
  board          [--style S] [--out credits.html]       every piece on one interactive HTML page
  site           [--out DIR] [--style S]                a small site: all pieces on the home page, one page per style (for GitHub Pages)
  publish        [--out credits.json]                   put the credits file at the repository root for publishing (lists what becomes public)

Every display command takes --credits FILE to use an existing credits file (CI has no agent logs; it uses the public credits.json in the repository)
  fetch                                                 terminal card

Every command takes --json and --lang zh|en (or set CREDITIA_LANG).`;
const USAGE = () => L(USAGE_ZH, USAGE_EN);

// ───────────────────────── 核心二进制 ─────────────────────────
const VERSION: string = JSON.parse(readFileSync(path.join(HERE, '../package.json'), 'utf8')).version;
const TARGETS: Record<string, string> = {
  'darwin-arm64': 'aarch64-apple-darwin', 'darwin-x64': 'x86_64-apple-darwin', 'linux-x64': 'x86_64-unknown-linux-gnu',
  'linux-arm64': 'aarch64-unknown-linux-gnu', 'win32-x64': 'x86_64-pc-windows-msvc',
};
const exeName = process.platform === 'win32' ? 'creditia-core.exe' : 'creditia-core';
/** 下载后的核心放在这里，按版本分开 */
const cachedCore = () => path.join(process.env.CREDITIA_HOME ?? path.join(process.env.HOME ?? process.env.USERPROFILE ?? '.', '.creditia'), 'bin', VERSION, exeName);

function corePath(): string | null {
  if (process.env.CREDITIA_CORE) return process.env.CREDITIA_CORE;
  for (const p of [path.join(ROOT, 'target/release', exeName), path.join(ROOT, 'target/debug', exeName), cachedCore()]) if (existsSync(p)) return p;
  const which = spawnSync(process.platform === 'win32' ? 'where' : 'which', [exeName], { encoding: 'utf8' });
  return which.status === 0 ? which.stdout.split('\n')[0].trim() : null;
}
/** 第一次用分析功能时，从 GitHub Releases 下载对应平台的核心（只下一次） */
async function ensureCore(): Promise<string> {
  const found = corePath();
  if (found) return found;
  const target = TARGETS[`${process.platform}-${process.arch}`];
  if (!target) die(L(`这个平台（${process.platform}-${process.arch}）没有预编译的核心；请从源码 cargo build --release，再设置 CREDITIA_CORE`, `no prebuilt core for this platform (${process.platform}-${process.arch}); build it with cargo build --release and set CREDITIA_CORE`));
  const url = `https://github.com/WroughtMind/creditia/releases/download/v${VERSION}/creditia-core-${target}${process.platform === 'win32' ? '.exe' : ''}`;
  if (!json) console.error(L(`第一次运行，下载分析核心：${url}`, `First run: downloading the analysis core: ${url}`));
  const res = await fetch(url);
  if (!res.ok) die(L(`下载失败（${res.status}）：${url}\n可以从源码 cargo build --release，再设置 CREDITIA_CORE`, `download failed (${res.status}): ${url}\nYou can build it with cargo build --release and set CREDITIA_CORE`));
  const out = cachedCore();
  mkdirSync(path.dirname(out), { recursive: true });
  writeFileSync(out, new Uint8Array(await res.arrayBuffer()), { mode: 0o755 });
  return out;
}
let coreBin: string | null = null;
function core(args: string[], inherit = true): { code: number; out: string } {
  const bin = coreBin ?? corePath();
  if (!bin) die(L('找不到分析核心 creditia-core', "can't find the analysis core creditia-core"));
  const r = spawnSync(bin, args, { stdio: inherit ? 'inherit' : ['ignore', 'pipe', 'inherit'], encoding: 'utf8', env: { ...process.env, CREDITIA_LANG: LANG() } });
  if (r.error) die(L(`运行 creditia-core 失败：${(r.error as any).message}`, `creditia-core failed to run: ${(r.error as any).message}`));
  return { code: r.status ?? 1, out: r.stdout ?? '' };
}
function die(msg: string): never {
  if (json) console.log(JSON.stringify({ error: msg }));
  else console.error(L(`错误：${msg}`, `error: ${msg}`));
  process.exit(1);
}

// ───────────────────────── 仓库与档案 ─────────────────────────
function sh(cwd: string, args: string[]) {
  const r = spawnSync('git', ['-C', cwd, ...args], { encoding: 'utf8' });
  return r.status === 0 ? r.stdout.trim() : '';
}
/** 仓库根：在 worktree 里运行也映射回主仓库 */
function repoOf(): string {
  const start = path.resolve(flags.repo ?? process.cwd());
  const top = sh(start, ['rev-parse', '--show-toplevel']);
  if (!top) {
    if (flags.credits) return start; // 只渲染一份现成的档案，不需要仓库
    die(L(`不是 git 仓库：${start}`, `not a git repository: ${start}`));
  }
  const common = sh(top, ['rev-parse', '--git-common-dir']);
  return common.endsWith('/.git') || common === '.git' ? path.dirname(path.resolve(top, common)) : top;
}
/** 档案从哪来：--credits 指定的文件 → 本机分析结果 .creditia/credits.json（必要时现场分析）→ 仓库根目录公开的 credits.json */
function loadCredits(repo: string, refresh = false): any {
  if (flags.credits) {
    const f = path.resolve(flags.credits);
    if (!existsSync(f)) die(L(`没有 ${f}`, `no such file: ${f}`));
    return JSON.parse(readFileSync(f, 'utf8'));
  }
  const pub = path.join(repo, 'credits.json');
  const p = path.join(repo, '.creditia/credits.json');
  if (!existsSync(p) && !refresh && existsSync(pub)) return JSON.parse(readFileSync(pub, 'utf8'));
  if (refresh || !existsSync(p)) {
    const r = core(['update', '--repo', repo, '--json'], false);
    if (r.code === 1) die(L('分析失败', 'analysis failed'));
  }
  if (!existsSync(p)) die(L(`没有 ${p}；先运行 creditia analyze`, `no ${p}; run creditia analyze first`));
  return JSON.parse(readFileSync(p, 'utf8'));
}
/** 能出图的作品：需要额外数据的（个人履历、逐行溯源）不在这里 */
const PIECE_IDS = [...Object.keys(FORMS).filter((id) => id !== 'profile' && id !== 'blame'), 'badge'];
/** 新风格里和原版重名、或者原版已经有对应的，用原版 */
const V2_SKIP = new Set(['minimal', 'noir', 'glass', 'cinema', 'terminal']);
const V2_STYLES = V2.STYLES.filter((s) => !V2_SKIP.has(s.id));
/** 风格：原版主题（Theme）或新风格（Style） */
type Look = { kind: 'classic'; t: Theme } | { kind: 'v2'; s: (typeof V2.STYLES)[number] };
function lookOf(id?: string): Look {
  const t = themeById(id ?? 'minimal-light');
  if (t) return { kind: 'classic', t };
  const s = V2_STYLES.find((x) => x.id === id);
  if (s) return { kind: 'v2', s };
  return die(L(`未知风格：${id}；creditia export --list 查看`, `unknown style: ${id}; see creditia export --list`));
}
const lookId = (l: Look) => (l.kind === 'classic' ? l.t.id : l.s.id);
const lookName = (l: Look, lang: 'zh' | 'en') => (l.kind === 'classic' ? themeName(l.t, lang) : lang === 'zh' ? l.s.name : l.s.en);
const lookBg = (l: Look) => (l.kind === 'classic' ? l.t.bg : l.s.c.bg);
const lookDark = (l: Look) => (l.kind === 'classic' ? l.t.dark : !!l.s.dark);
const lookDot = (l: Look) => (l.kind === 'classic' ? themeDot(l.t) : l.s.dot());
const exOf = (l: Look) => (l.kind === 'classic' ? ex : exV2);
/** 新风格的 hand 只写人这一侧的数字，作品里不再写人 */
const V2_PIECES = Object.keys(V2.PIECES).filter((id) => id !== 'hand');
const piecesOf = (l: Look) => (l.kind === 'classic' ? PIECE_IDS : V2_PIECES);
const LOOKS: Look[] = [...ORDERED_THEMES.map((t) => ({ kind: 'classic' as const, t })), ...V2_STYLES.map((s) => ({ kind: 'v2' as const, s }))];
async function artOf(id: string, raw: any, l: Look, variant?: string): Promise<Art | null> {
  if (l.kind === 'classic') return art(id, buildModel(raw, { director: false, lang: LANG() }), l.t, ctxOf(variant));
  return V2.art(id, V2.buildModel(raw, { lang: LANG(), named: false, sign: '' }), l.s, { variant });
}
const ctxOf = (variant?: string) => ({ lang: LANG(), variant });
/** 文字类输出（CREDITS.md、发版说明、PR 评论、终端名片）用新模型：带模型名、好读的模型名 */
const textModel = (raw: any) => V2.buildModel(raw, { lang: LANG(), named: false, sign: '' });
function modelOf(repo: string, refresh = false): { m: Model; raw: any } {
  const raw = loadCredits(repo, refresh);
  const m = buildModel(raw, { director: false, lang: LANG() });
  return { m, raw };
}
function write(out: string | undefined, data: string | Uint8Array) {
  if (!out) { process.stdout.write(data); return; }
  mkdirSync(path.dirname(path.resolve(out)), { recursive: true });
  writeFileSync(out, data);
  if (json) console.log(JSON.stringify({ path: path.resolve(out), bytes: typeof data === 'string' ? Buffer.byteLength(data) : data.length }));
  else console.error(`→ ${out}`);
}

// ───────────────────────── 命令 ─────────────────────────
const PASS = new Set(['discover', 'init', 'analyze', 'update', 'collect', 'doctor', 'blame', 'projects', 'stamp', 'hook', 'hook-trailers', 'paths', 'link', 'unlink', 'whoami', 'evidence']);

async function main() {
  if (!cmd || cmd === 'help' || cmd === '-h' || cmd === '--help') return console.log(USAGE());
  if (cmd === '--version' || cmd === 'version') return console.log(VERSION);
  // 只有要做分析时才需要核心：透传给核心的命令、--refresh、或者仓库里还没有任何档案
  const top = sh(path.resolve(flags.repo ?? process.cwd()), ['rev-parse', '--show-toplevel']);
  const hasCredits = !!flags.credits || (!!top && (existsSync(path.join(top, '.creditia/credits.json')) || existsSync(path.join(top, 'credits.json'))));
  if (PASS.has(cmd) || on.has('refresh') || (!hasCredits && !on.has('list'))) coreBin = await ensureCore();
  if (PASS.has(cmd)) process.exit(core(argv).code);
  switch (cmd) {
    case 'doc': {
      const repo = repoOf();
      const { m, raw } = modelOf(repo, on.has('refresh'));
      const out = flags.out ?? path.join(repo, 'CREDITS.md');
      const existing = existsSync(out) ? readFileSync(out, 'utf8') : null;
      writeFileSync(out, mergeCreditsMd(existing, creditsBlock(textModel(raw))));
      console.log(json ? JSON.stringify({ path: out, project: m.project, updated: existing !== null }) : `${existing ? L('已更新', 'Updated') : L('已写入', 'Wrote')} ${out}`);
      return;
    }
    case 'release-notes': { const { raw } = modelOf(repoOf()); return write(flags.out, releaseNotes(textModel(raw), pos[0] ?? 'unreleased')); }
    case 'pr-comment': {
      const id = pos[0] ? (/^\d+$/.test(pos[0]) ? `pr:${pos[0]}` : pos[0]) : die(L('需要 PR 编号', 'a PR number is required'));
      const { raw } = modelOf(repoOf());
      const s = prComment(textModel(raw), id);
      if (!s) die(L(`没有找到 ${id}`, `not found: ${id}`));
      return write(flags.out, s);
    }
    case 'export': return exportCmd();
    case 'board': return boardCmd();
    case 'site': return siteCmd();
    case 'publish': return publishCmd();
    case 'fetch': { const { raw } = modelOf(repoOf()); console.log(ansi(fetchSegs(textModel(raw)))); return; }
    default: die(L(`未知命令：${cmd}\n\n${USAGE()}`, `unknown command: ${cmd}\n\n${USAGE()}`));
  }
}

const FORMAT_EN: Record<string, string> = { png: 'social posts, chats, documents', svg: 'README, web pages, lossless scaling', pdf: 'print, résumés, portfolios', gif: 'animated: bars grow, credits roll', html: 'single file, hover for details, works offline', md: 'CREDITS.md block' };

function listAll() {
  const lang = LANG();
  const pieces = PIECE_IDS.map((id) => ({ id, name: pieceName(id, lang), question: pieceQ(id, lang), variants: FORMS[id]?.variants ?? [] }));
  const v2pieces = Object.entries(V2.PIECES).filter(([id]) => id !== 'hand').map(([id, p]) => ({ id, name: lang === 'en' ? p.en : p.name, question: lang === 'en' ? p.qEn : p.q, variants: p.variants ?? [], animated: !!p.animated }));
  const styles = LOOKS.map((l) => ({ id: lookId(l), name: lookName(l, 'zh'), en: lookName(l, 'en'), renderer: l.kind, note: l.kind === 'classic' ? themeNote(l.t, lang) : lang === 'en' ? l.s.noteEn : l.s.note }));
  const formats = FORMATS.filter((f) => f.id !== 'embed').map((f) => ({ ...f, note: lang === 'en' ? FORMAT_EN[f.id] ?? f.note : f.note }));
  if (json) return console.log(JSON.stringify({ pieces, v2pieces, styles, formats: [...formats.map((f) => f.id), 'json'], sizes: SIZES.map((s) => s.id) }));
  const nameOf = (s: (typeof styles)[number]) => (lang === 'en' ? s.en : s.name);
  console.log(L('作品（原版主题）', 'Pieces (classic themes)'));
  for (const p of pieces) console.log(`  ${p.id.padEnd(10)} ${p.name.padEnd(lang === 'en' ? 16 : 8)} ${p.question}${p.variants.length ? `\n${''.padEnd(13)}${L('版式：', 'layouts: ')}${p.variants.map((v) => L(`${v}（${variantName(v, 'zh')}）`, `${v} (${variantName(v, 'en')})`)).join(' ')}` : ''}`);
  console.log(L('\n原版主题', '\nClassic themes'));
  for (const s of styles.filter((x) => x.renderer === 'classic')) console.log(`  ${s.id.padEnd(14)} ${nameOf(s).padEnd(lang === 'en' ? 14 : 6)} ${s.note}`);
  console.log(L('\n新风格（作品：', '\nNewer styles (pieces: ') + v2pieces.map((p) => p.id).join(' ') + L('；gif 只在会动的作品上）', '; gif only for animated pieces)'));
  for (const s of styles.filter((x) => x.renderer === 'v2')) console.log(`  ${s.id.padEnd(14)} ${nameOf(s).padEnd(lang === 'en' ? 14 : 6)} ${s.note}`);
  console.log(`\n${L('格式', 'formats')}  ${formats.map((f) => L(`${f.id}（${f.note}）`, `${f.id} (${f.note})`)).join('  ')}`);
  console.log(`${L('尺寸', 'sizes')}  ${SIZES.map((s) => s.id).join(' ')} ${L('或 1200x630 这样的宽x高', 'or a width x height like 1200x630')}`);
}

async function exportCmd() {
  if (on.has('list')) return listAll();
  const repo = repoOf();
  const piece = flags.piece ?? flags.form ?? 'tier';
  const look = lookOf(flags.style ?? flags.theme);
  const e = exOf(look);
  const format = flags.format ?? (flags.out ? path.extname(flags.out).slice(1) : 'svg') ?? 'svg';
  const { m, raw } = modelOf(repo, on.has('refresh'));
  if (format === 'json') return write(flags.out, JSON.stringify(raw, null, 2) + '\n');
  if (format === 'md') return write(flags.out, creditsBlock(textModel(raw)) + '\n');
  if (!piecesOf(look).includes(piece)) die(L(`这个风格没有作品 ${piece}；creditia export --list 查看`, `this style has no piece ${piece}; see creditia export --list`));
  const a0 = await artOf(piece, raw, look, flags.variant);
  if (!a0) die(L(`${piece} 需要的数据不够（例如没有 tag、没有会话记录），没有出图`, `not enough data for ${piece} (for example no tags or no sessions); nothing rendered`));
  const fit = (a: Art): Art => {
    if (!flags.size) return a;
    const preset = SIZES.find((s) => s.id === flags.size);
    const [w, h] = preset ? [preset.w, preset.h] : flags.size.split(/[x×]/).map(Number);
    if (!w || !h) die(L('--size 形如 og、square、1200x630', '--size looks like og, square or 1200x630'));
    return sized(a, w, h);
  };
  const a = fit(a0);
  switch (format) {
    case 'svg': return write(flags.out, e.svg(a));
    case 'png': return write(flags.out, e.png(a, flags.size ? 1 : Number(flags.scale ?? 2)));
    case 'pdf': return write(flags.out, await e.pdf(a, flags.size === 'a4' ? 1 : 3));
    case 'html': return write(flags.out, e.html([a0], { title: a0.title, lang: LANG() }));
    case 'gif': {
      if (look.kind !== 'v2' || !V2.PIECES[piece]?.animated) die(L(`${piece} 在这个风格下不会动；换 png`, `${piece} isn't animated in this style; use png`));
      const fr = V2.frames(piece, V2.buildModel(raw, { lang: LANG(), named: false, sign: '' }), look.s, { variant: flags.variant }, Number(flags.frames ?? 24)).map(fit);
      return write(flags.out, e.gif(fr, { width: Number(flags.width ?? Math.min(fr[0].w, 800)) }));
    }
    default: die(L(`未知格式：${format}`, `unknown format: ${format}`));
  }
}

const boardIds = (l: Look) => (l.kind === 'classic'
  ? ['share', 'tier', 'roll', 'receipt', 'donut', 'ribbon', 'waffle', 'mosaic', 'versions', 'area', 'lanes', 'clock', 'settle']
  : ['share', 'tier', 'roll', 'receipt', 'donut', 'stream', 'survival', 'versions', 'lanes', 'clock', 'mosaic', 'codemap', 'calendar']);
async function artsOf(ids: string[], raw: any, l: Look): Promise<Art[]> {
  const out: Art[] = [];
  for (const id of ids) if (piecesOf(l).includes(id)) { const a = await artOf(id, raw, l); if (a) out.push(a); }
  return out;
}

/** 一页放下全部作品：可交互、可离线，适合放进项目官网的 /credits */
async function boardCmd() {
  const repo = repoOf();
  const look = lookOf(flags.style);
  const { m, raw } = modelOf(repo, on.has('refresh'));
  const arts = await artsOf((flags.pieces ?? boardIds(look).join(',')).split(','), raw, look);
  write(flags.out ?? path.join(repo, 'credits.html'), exOf(look).html(arts, { title: `${m.project} · credits`, lang: LANG(), bg: lookBg(look), note: m.project }));
}

/** 公开档案：从本机分析结果复制到仓库根目录，并列出将公开的内容让人确认 */
function publishCmd() {
  const repo = repoOf();
  const { m, raw } = modelOf(repo, on.has('refresh'));
  const out = flags.out ?? path.join(repo, 'credits.json');
  // 隐藏标题时 PR 写成「PR #12」、提交写成短哈希：都不算标题
  const titles = (raw.units ?? []).filter((u: any) => u.title && !/^PR #\d+$/.test(u.title) && !/^[0-9a-f]{7}$/.test(u.title)).length;
  const paths = (raw.files ?? []).filter((f: any) => f.path).length;
  // 作品里不写人名；档案里仍有提交者的显示名，公开前列出来
  const people = [...new Set((raw.contributors ?? []).filter((c: any) => c.kind === 'human' && c.name).map((c: any) => c.name as string))];
  const report = { path: out, project: m.project, prTitles: titles, filePaths: paths, people, note: L('只含行数、工具、模型、时间与上面这些字段；不含代码、prompt、邮箱、本机路径', 'Only line counts, tools, models, times and the fields above; no code, prompts, emails or local paths') };
  writeFileSync(out, JSON.stringify(raw, null, 1) + '\n');
  if (json) return console.log(JSON.stringify(report));
  console.log(L(
    `已写入 ${out}\n将公开：项目名「${m.project}」、${titles} 个 PR / 提交标题、${paths} 个文件路径、人名：${people.join('、') || '无'}\n${report.note}\n不想公开标题、路径或人名：改 .creditia/config.json 的 privacy 后重新 analyze。`,
    `Wrote ${out}\nThis makes public: the project name "${m.project}", ${titles} PR / commit titles, ${paths} file paths, people's names: ${people.join(', ') || 'none'}\n${report.note}\nTo keep titles, paths or names private: change privacy in .creditia/config.json and analyze again.`,
  ));
}

/** 小网站：首页是默认风格的全部作品，每种风格一页（四件代表作），顶部一排风格圆点 */
async function siteCmd() {
  const repo = repoOf();
  const { m, raw } = modelOf(repo, on.has('refresh'));
  const out = flags.out ?? path.join(repo, '_credits');
  mkdirSync(out, { recursive: true });
  const main = lookOf(flags.style);
  const lang = LANG();
  const nav = (cur: string) => `<nav style="position:sticky;top:0;z-index:5;display:flex;flex-wrap:wrap;gap:8px;justify-content:center;padding:14px;backdrop-filter:blur(16px);-webkit-backdrop-filter:blur(16px)">${LOOKS.map((l) => `<a href="${lookId(l) === lookId(main) ? 'index' : lookId(l)}.html" title="${lookName(l, lang)}" style="display:block;width:22px;height:22px;border-radius:99px;${lookId(l) === cur ? 'outline:2px solid currentColor;outline-offset:3px' : ''}"><svg viewBox="0 0 24 24" width="22" height="22">${lookDot(l)}</svg></a>`).join('')}</nav>`;
  const page = async (l: Look, ids: string[]) => {
    const arts = await artsOf(ids, raw, l);
    return exOf(l).html(arts, { title: `${m.project} · credits · ${lookName(l, lang)}`, lang, bg: lookBg(l), note: `${m.project} · ${lookName(l, lang)}` }).replace('<body', `<body style="color:${lookDark(l) ? '#eee' : '#111'}"`).replace(/(<body[^>]*>)/, `$1${nav(lookId(l))}`);
  };
  writeFileSync(path.join(out, 'index.html'), await page(main, boardIds(main)));
  for (const l of LOOKS) if (lookId(l) !== lookId(main)) writeFileSync(path.join(out, `${lookId(l)}.html`), await page(l, ['share', 'tier', 'roll', 'receipt']));
  if (json) console.log(JSON.stringify({ path: out, pages: LOOKS.length }));
  else console.error(L(`→ ${out}/index.html（${LOOKS.length} 页）`, `→ ${out}/index.html (${LOOKS.length} pages)`));
}

// ───────────────────────── 终端颜色 ─────────────────────────
function ansi(lines: { text: string; color?: string; dim?: boolean; bold?: boolean }[][]): string {
  const color = !on.has('no-color') && !process.env.NO_COLOR && process.stdout.isTTY;
  const rgb = (hex: string) => { const r = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})/i.exec(hex); return r ? `\x1b[38;2;${parseInt(r[1], 16)};${parseInt(r[2], 16)};${parseInt(r[3], 16)}m` : ''; };
  return lines.map((ln) => ln.map((s) => {
    if (!color) return s.text;
    let pre = '';
    if (s.bold) pre += '\x1b[1m';
    if (s.dim) pre += '\x1b[2m';
    if (s.color && BRAND[s.color]) pre += rgb(BRAND[s.color]);
    return pre ? `${pre}${s.text}\x1b[0m` : s.text;
  }).join('')).join('\n');
}

main().catch((e) => die(e?.message ?? String(e)));
