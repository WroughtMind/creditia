// creditia CLI：Rust 核心负责分析（透传），这里负责档案、导出、终端展示、打开 App。
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { FORMS, renderSvg, renderSized, svgToPng, buildModel, THEMES, themeById, type Ctx, type Model } from '../../render/src/index.ts';
import { badge } from '../../render/src/forms.ts';
import { creditsBlock, mergeCreditsMd, releaseNotes, prComment, fetchSegs } from '../../render/src/text.ts';
import { BRANDS } from '../../render/src/brand.ts';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '../../..');

// ───────────────────────── 参数 ─────────────────────────
const SWITCHES = new Set(['json', 'fresh', 'force', 'github', 'all', 'yes', 'list', 'director', 'no-color', 'refresh']);
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

分析（由 Rust 核心完成）
  discover                          列出本机有 agent 活动的项目
  init      [--repo P]              在仓库写 .creditia/config.json
  analyze   [--repo P] [--github]   分析，写 .creditia/credits.json
  update    [--repo P]              增量更新
  collect   [--repo P | --all]      只采集证据
  doctor                            检查各数据源
  blame     <路径>:<行号>           这一行是谁写的
  stamp     --harness H [--model M] 给下一次提交盖章
  hook      install|uninstall       提交盖章 hook
  projects                          作品库里的项目

档案与展示
  doc            [--repo P] [--out CREDITS.md]           生成或更新 CREDITS.md（只改标记区间）
  release-notes  <tag|unreleased>                        该版本的 credits 段落
  pr-comment     <N>                                     该 PR 的一句话归因
  export         --form F [--theme T] [--variant V] [--format F] [--out FILE] [--size WxH] [--lang zh|en] [--director]
  export         --list                                  全部形式和风格
  fetch          [--repo P]                              终端名片
  app            open [--form F] [--theme T]             在 App 里打开

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
function repoOf(): string {
  const start = path.resolve(flags.repo ?? process.cwd());
  const top = sh(start, ['rev-parse', '--show-toplevel']);
  if (!top) die(`不是 git 仓库：${start}`);
  const common = sh(top, ['rev-parse', '--git-common-dir']);
  const main = common.endsWith('/.git') ? path.dirname(path.resolve(top, common)) : top;
  return main;
}
function loadConfig(repo: string): any {
  const p = path.join(repo, '.creditia/config.json');
  return existsSync(p) ? JSON.parse(readFileSync(p, 'utf8')) : {};
}
function loadCredits(repo: string, refresh = false): any {
  const p = path.join(repo, '.creditia/credits.json');
  if (refresh || !existsSync(p)) {
    const r = core(['update', '--repo', repo, '--json'], false);
    if (r.code === 1) die('分析失败');
  }
  if (!existsSync(p)) die(`没有 ${p}；先运行 creditia analyze`);
  return JSON.parse(readFileSync(p, 'utf8'));
}
function modelOf(repo: string, refresh = false): { m: Model; cfg: any; raw: any } {
  const cfg = loadConfig(repo);
  const raw = loadCredits(repo, refresh);
  const m = buildModel(raw, { director: on.has('director') || !!cfg.display?.director, lang: (flags.lang as 'zh' | 'en') ?? 'zh' });
  return { m, cfg, raw };
}
function write(out: string | undefined, data: string | Buffer) {
  if (!out) {
    process.stdout.write(data);
    return;
  }
  mkdirSync(path.dirname(path.resolve(out)), { recursive: true });
  writeFileSync(out, data);
}

// ───────────────────────── 命令 ─────────────────────────
const PASS = new Set(['discover', 'init', 'analyze', 'update', 'collect', 'doctor', 'blame', 'projects', 'stamp', 'hook', 'hook-trailers']);

async function main() {
  if (!cmd || cmd === 'help' || cmd === '-h' || cmd === '--help') return console.log(USAGE);
  if (PASS.has(cmd)) process.exit(core(argv).code);

  switch (cmd) {
    case 'doc': {
      const repo = repoOf();
      const { m } = modelOf(repo, on.has('refresh'));
      const out = flags.out ?? path.join(repo, 'CREDITS.md');
      const existing = existsSync(out) ? readFileSync(out, 'utf8') : null;
      const text = mergeCreditsMd(existing, creditsBlock(m));
      writeFileSync(out, text);
      const res = { path: out, project: m.project, updated: existing !== null };
      console.log(json ? JSON.stringify(res) : `${existing ? '已更新' : '已写入'} ${out}`);
      return;
    }
    case 'release-notes': {
      const tag = pos[0] ?? 'unreleased';
      const { m } = modelOf(repoOf());
      write(flags.out, releaseNotes(m, tag));
      return;
    }
    case 'pr-comment': {
      const id = pos[0] ? (/^\d+$/.test(pos[0]) ? `pr:${pos[0]}` : pos[0]) : die('需要 PR 编号');
      const { m } = modelOf(repoOf());
      const s = prComment(m, id);
      if (!s) die(`没有找到 ${id}`);
      write(flags.out, s);
      return;
    }
    case 'export':
      return exportCmd();
    case 'fetch': {
      const { m } = modelOf(repoOf());
      console.log(ansi(fetchSegs(m)));
      return;
    }
    case 'app': {
      const repo = repoOf();
      const q = new URLSearchParams({ repo, form: flags.form ?? 'tier', theme: flags.theme ?? 'minimal-light' });
      const url = `creditia://open?${q}`;
      const opener = process.platform === 'darwin' ? ['open', url] : process.platform === 'win32' ? ['cmd', '/c', 'start', '', url] : ['xdg-open', url];
      const r = spawnSync(opener[0], opener.slice(1), { stdio: 'ignore' });
      const ok = r.status === 0;
      console.log(json ? JSON.stringify({ url, opened: ok }) : ok ? `已打开 ${url}` : `无法打开 ${url}；请安装 creditia App，或用 creditia export 导出图片`);
      return;
    }
    default:
      die(`未知命令：${cmd}\n\n${USAGE}`);
  }
}

async function exportCmd() {
  if (on.has('list')) {
    const rows = Object.entries(FORMS).map(([id, f]) => `${id.padEnd(10)} ${f.name.padEnd(8)} ${f.q.padEnd(16)} ${f.size}${f.variants ? `  版式：${f.variants.join(' / ')}` : ''}`);
    const themes = THEMES.map((t) => `${t.id.padEnd(14)} ${t.name}`);
    if (json) return console.log(JSON.stringify({ forms: Object.entries(FORMS).map(([id, f]) => ({ id, name: f.name, question: f.q, size: f.size, variants: f.variants ?? [] })), themes: THEMES.map((t) => ({ id: t.id, name: t.name })) }));
    console.log(`形式\n${rows.join('\n')}\n\n风格\n${themes.join('\n')}\n\n格式：svg png embed html md json（徽章：--form badge --format svg|md）`);
    return;
  }
  const repo = repoOf();
  const form = flags.form ?? 'tier';
  const theme = themeById(flags.theme ?? 'minimal-light') ?? die(`未知风格：${flags.theme}；creditia export --list 查看`);
  const format = flags.format ?? (form === 'badge' ? 'svg' : 'svg');
  const { m, raw } = modelOf(repo, on.has('refresh'));
  const lang = (flags.lang as 'zh' | 'en') ?? 'zh';

  if (form === 'badge') {
    const svg = badge(m, theme);
    if (format === 'md') {
      const file = flags.out ?? 'assets/creditia-badge.svg';
      write(file, svg);
      console.log(`![built with](${file})`);
      return;
    }
    return write(flags.out, svg);
  }
  if (format === 'json') return write(flags.out, JSON.stringify(raw, null, 2) + '\n');
  if (format === 'md') return write(flags.out, creditsBlock(m) + '\n');
  if (format === 'embed') {
    const src = flags.src ?? '/.creditia/credits.json';
    const snippet = `<script type="module" src="https://cdn.jsdelivr.net/npm/@creditia/embed@0/dist/creditia-card.js"></script>\n<creditia-card src="${src}" form="${form}" theme="${theme.id}"${on.has('director') ? ' director' : ''}></creditia-card>\n`;
    if (json) return console.log(JSON.stringify({ html: snippet, src, form, theme: theme.id }));
    return write(flags.out, snippet);
  }
  if (!FORMS[form]) die(`未知形式：${form}；creditia export --list 查看`);

  // 需要额外数据的形式
  const ctx: Ctx = { lang, variant: flags.variant };
  if (form === 'profile') {
    const r = core(['projects', '--json'], false);
    const list: any[] = r.code === 1 ? [] : JSON.parse(r.out).projects ?? [];
    ctx.projects = list.map((p) => path.join(p.path, '.creditia/credits.json')).filter(existsSync).map((p) => buildModel(JSON.parse(readFileSync(p, 'utf8')), { director: false, lang }));
    if (!ctx.projects.length) ctx.projects = [m];
    ctx.person = raw.directors?.[0]?.name ?? '';
  }
  if (form === 'blame') {
    const q = flags.line ?? die('逐行溯源需要 --line <路径>:<行号>');
    const r = core(['blame', q, '--repo', repo, '--json'], false);
    if (r.code === 1) die('blame 失败');
    ctx.blame = JSON.parse(r.out);
  }

  let svg: string | null;
  if (flags.size) {
    const [w, h] = flags.size.split(/[x×]/).map(Number);
    if (!w || !h) die('--size 形如 1200x630');
    svg = await renderSized(form, m, theme, w, h, ctx);
  } else {
    svg = await renderSvg(form, m, theme, ctx);
  }
  if (!svg) die(`${form} 需要的数据不够（例如没有 tag、没有会话记录），没有出图`);
  if (format === 'svg') return write(flags.out, svg);
  if (format === 'png') return write(flags.out, svgToPng(svg, flags.width ? Number(flags.width) : undefined));
  if (format === 'html') return write(flags.out, `<!doctype html><meta charset="utf-8"><style>body{margin:0;display:grid;place-items:center;min-height:100vh;background:${theme.bg}}svg{max-width:100%;height:auto}</style>${svg}`);
  die(`未知格式：${format}`);
}

// ───────────────────────── 终端颜色 ─────────────────────────
function ansi(lines: { text: string; color?: string; dim?: boolean; bold?: boolean }[][]): string {
  const color = !on.has('no-color') && !process.env.NO_COLOR && process.stdout.isTTY;
  const rgb = (hex: string) => {
    const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})/i.exec(hex);
    return m ? `\x1b[38;2;${parseInt(m[1], 16)};${parseInt(m[2], 16)};${parseInt(m[3], 16)}m` : '';
  };
  return lines
    .map((ln) =>
      ln
        .map((s) => {
          if (!color) return s.text;
          let pre = '';
          if (s.bold) pre += '\x1b[1m';
          if (s.dim) pre += '\x1b[2m';
          if (s.color && BRANDS[s.color]) pre += rgb(BRANDS[s.color].color);
          return pre ? `${pre}${s.text}\x1b[0m` : s.text;
        })
        .join(''),
    )
    .join('\n');
}

main().catch((e) => die(e?.message ?? String(e)));
