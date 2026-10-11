// creditia CLI：分析交给 Rust 核心（透传），这里负责档案、导出、终端名片。
// 所有命令非交互、支持 --json，方便 agent 代人调用。
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { deflateSync } from 'node:zlib';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { PIECES, art, frames, variantName, buildModel, STYLES, styleById, raster, type Model } from '../../render/src/index.ts';
import { makeExporter, sized, SIZES, FORMATS, type Art } from '../../render/src/export.ts';
import { creditsBlock, mergeCreditsMd, releaseNotes, prComment, fetchSegs, embedSnippet } from '../../render/src/text.ts';
import { BRAND } from '../../render/src/style.ts';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '../../..');
const ex = makeExporter(raster, (d) => new Uint8Array(deflateSync(d)));

// ───────────────────────── 参数 ─────────────────────────
const SWITCHES = new Set(['json', 'fresh', 'force', 'github', 'all', 'yes', 'list', 'named', 'no-color', 'refresh']);
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

const USAGE = `用法：creditia <命令> [选项]

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
  export         --piece P [--style S] [--variant V] [--format png|svg|pdf|gif|html|md|embed|json]
                 [--size og|square|portrait|story|a4|WxH] [--lang zh|en] [--named] [--sign 名字] [--out FILE]
  export         --list                                 全部作品、版式、风格、格式
  board          [--style S] [--out credits.html]       全部作品放进一个可交互的 HTML 页面
  fetch                                                 终端名片

所有命令支持 --json。`;

// ───────────────────────── 核心二进制 ─────────────────────────
function corePath(): string {
  if (process.env.CREDITIA_CORE) return process.env.CREDITIA_CORE;
  const exe = process.platform === 'win32' ? 'creditia-core.exe' : 'creditia-core';
  for (const p of [path.join(ROOT, 'target/release', exe), path.join(ROOT, 'target/debug', exe), path.join(HERE, '..', 'bin', `${process.platform}-${process.arch}`, exe)]) if (existsSync(p)) return p;
  return exe;
}
function core(args: string[], inherit = true): { code: number; out: string } {
  const r = spawnSync(corePath(), args, { stdio: inherit ? 'inherit' : ['ignore', 'pipe', 'inherit'], encoding: 'utf8' });
  if (r.error) die(`找不到 creditia-core：${(r.error as any).message}\n先 cargo build --release，或设置 CREDITIA_CORE`);
  return { code: r.status ?? 1, out: r.stdout ?? '' };
}
function die(msg: string): never {
  if (json) console.log(JSON.stringify({ error: msg }));
  else console.error(`错误：${msg}`);
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
  if (!top) die(`不是 git 仓库：${start}`);
  const common = sh(top, ['rev-parse', '--git-common-dir']);
  return common.endsWith('/.git') || common === '.git' ? path.dirname(path.resolve(top, common)) : top;
}
const loadConfig = (repo: string): any => { const p = path.join(repo, '.creditia/config.json'); return existsSync(p) ? JSON.parse(readFileSync(p, 'utf8')) : {}; };
function loadCredits(repo: string, refresh = false): any {
  const p = path.join(repo, '.creditia/credits.json');
  if (refresh || !existsSync(p)) {
    const r = core(['update', '--repo', repo, '--json'], false);
    if (r.code === 1) die('分析失败');
  }
  if (!existsSync(p)) die(`没有 ${p}；先运行 creditia analyze`);
  return JSON.parse(readFileSync(p, 'utf8'));
}
function modelOf(repo: string, refresh = false): { m: Model; raw: any } {
  const cfg = loadConfig(repo);
  const raw = loadCredits(repo, refresh);
  const m = buildModel(raw, { lang: (flags.lang as 'zh' | 'en') ?? 'zh', named: on.has('named') || !!cfg.display?.named, sign: flags.sign ?? cfg.display?.sign ?? '' });
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
  if (!cmd || cmd === 'help' || cmd === '-h' || cmd === '--help') return console.log(USAGE);
  if (PASS.has(cmd)) process.exit(core(argv).code);
  switch (cmd) {
    case 'doc': {
      const repo = repoOf();
      const { m } = modelOf(repo, on.has('refresh'));
      const out = flags.out ?? path.join(repo, 'CREDITS.md');
      const existing = existsSync(out) ? readFileSync(out, 'utf8') : null;
      writeFileSync(out, mergeCreditsMd(existing, creditsBlock(m)));
      console.log(json ? JSON.stringify({ path: out, project: m.project, updated: existing !== null }) : `${existing ? '已更新' : '已写入'} ${out}`);
      return;
    }
    case 'release-notes': { const { m } = modelOf(repoOf()); return write(flags.out, releaseNotes(m, pos[0] ?? 'unreleased')); }
    case 'pr-comment': {
      const id = pos[0] ? (/^\d+$/.test(pos[0]) ? `pr:${pos[0]}` : pos[0]) : die('需要 PR 编号');
      const { m } = modelOf(repoOf());
      const s = prComment(m, id);
      if (!s) die(`没有找到 ${id}`);
      return write(flags.out, s);
    }
    case 'export': return exportCmd();
    case 'board': return boardCmd();
    case 'fetch': { const { m } = modelOf(repoOf()); console.log(ansi(fetchSegs(m))); return; }
    default: die(`未知命令：${cmd}\n\n${USAGE}`);
  }
}

function listAll() {
  const pieces = Object.entries(PIECES).map(([id, p]) => ({ id, name: p.name, question: p.q, variants: p.variants ?? [], animated: !!p.animated }));
  const styles = STYLES.map((s) => ({ id: s.id, name: s.name, en: s.en, note: s.note }));
  if (json) return console.log(JSON.stringify({ pieces, styles, formats: FORMATS.map((f) => f.id), sizes: SIZES.map((s) => s.id) }));
  console.log('作品');
  for (const p of pieces) console.log(`  ${p.id.padEnd(10)} ${p.name.padEnd(8)} ${p.question}${p.variants.length ? `\n${''.padEnd(13)}版式：${p.variants.map((v) => `${v}（${variantName(v, 'zh')}）`).join(' ')}` : ''}`);
  console.log('\n风格');
  for (const s of styles) console.log(`  ${s.id.padEnd(11)} ${s.name.padEnd(6)} ${s.note}`);
  console.log(`\n格式  ${FORMATS.map((f) => `${f.id}（${f.note}）`).join('  ')}`);
  console.log(`尺寸  ${SIZES.map((s) => s.id).join(' ')} 或 1200x630 这样的宽x高`);
}

async function exportCmd() {
  if (on.has('list')) return listAll();
  const repo = repoOf();
  const piece = flags.piece ?? flags.form ?? 'tier';
  const st = styleById(flags.style ?? flags.theme ?? 'minimal') ?? die(`未知风格：${flags.style ?? flags.theme}；creditia export --list 查看`);
  const format = flags.format ?? (flags.out ? path.extname(flags.out).slice(1) : 'svg') ?? 'svg';
  const { m, raw } = modelOf(repo, on.has('refresh'));
  if (format === 'json') return write(flags.out, JSON.stringify(raw, null, 2) + '\n');
  if (format === 'md') return write(flags.out, creditsBlock(m) + '\n');
  if (format === 'embed') return write(flags.out, embedSnippet(piece, st.id, flags.src, flags.variant));
  if (!PIECES[piece]) die(`未知作品：${piece}；creditia export --list 查看`);
  const a0 = art(piece, m, st, { variant: flags.variant });
  if (!a0) die(`${piece} 需要的数据不够（例如没有 tag、没有会话记录），没有出图`);
  const fit = (a: Art): Art => {
    if (!flags.size) return a;
    const preset = SIZES.find((s) => s.id === flags.size);
    const [w, h] = preset ? [preset.w, preset.h] : flags.size.split(/[x×]/).map(Number);
    if (!w || !h) die('--size 形如 og、square、1200x630');
    return sized(a, w, h);
  };
  const a = fit(a0);
  switch (format) {
    case 'svg': return write(flags.out, ex.svg(a));
    case 'png': return write(flags.out, ex.png(a, flags.size ? 1 : Number(flags.scale ?? 2)));
    case 'pdf': return write(flags.out, await ex.pdf(a, flags.size === 'a4' ? 1 : 3));
    case 'html': return write(flags.out, ex.html([a0], { title: a0.title, lang: m.lang }));
    case 'gif': {
      if (!PIECES[piece].animated) die(`${piece} 没有动画；换 png`);
      const fr = frames(piece, m, st, { variant: flags.variant }, Number(flags.frames ?? 24)).map(fit);
      return write(flags.out, ex.gif(fr, { width: Number(flags.width ?? Math.min(fr[0].w, 800)) }));
    }
    default: die(`未知格式：${format}`);
  }
}

/** 一页放下全部作品：可交互、可离线，适合放进项目官网的 /credits */
async function boardCmd() {
  const repo = repoOf();
  const st = styleById(flags.style ?? 'minimal') ?? die(`未知风格：${flags.style}`);
  const { m } = modelOf(repo, on.has('refresh'));
  const ids = (flags.pieces ?? 'share,tier,roll,hand,receipt,donut,stream,survival,versions,lanes,clock,mosaic,calendar').split(',');
  const arts = ids.map((id) => (PIECES[id] ? art(id, m, st) : null)).filter(Boolean) as Art[];
  write(flags.out ?? path.join(repo, 'credits.html'), ex.html(arts, { title: `${m.project} · credits`, lang: m.lang, bg: st.c.bg, note: m.project }));
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
