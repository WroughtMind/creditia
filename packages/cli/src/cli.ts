// creditia CLI：Rust 核心负责分析（透传），这里负责档案、导出、终端展示、打开 App。
import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
import { homedir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { FORMS, renderSvg, renderSized, svgToRaster, buildModel, THEMES, themeById, type Ctx, type Model } from '../../render/src/index.ts';
import { encodeExport, EXPORTS, type ExportFormat } from '../../render/src/export.ts';
import { badge } from '../../render/src/forms.ts';
import { creditsBlock, mergeCreditsMd, releaseNotes, prComment, fetchSegs } from '../../render/src/text.ts';
import { BRANDS } from '../../render/src/brand.ts';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '../../..');

// ───────────────────────── 参数 ─────────────────────────
const SWITCHES = new Set(['json', 'fresh', 'force', 'github', 'all', 'yes', 'list', 'people', 'show', 'no-color', 'refresh']);
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
  link      --path P [--repo P]   关联同一项目的副本或旧路径
  people    --file FILE [--show]   保存用户确认的人员署名

档案与展示
  doc            [--repo P] [--out CREDITS.md]           生成或更新 CREDITS.md（只改标记区间）
  release-notes  <tag|unreleased>                        该版本的 credits 段落
  pr-comment     <N>                                     该 PR 的一句话归因
  export         --form F [--theme T] [--variant V] [--format svg|png|pdf|gif|html|md|json] [--out FILE] [--size WxH] [--lang zh|en] [--people]
  export         --list                                  全部形式和风格
  fetch          [--repo P]                              终端名片
  app            open [--form F] [--theme T]             在 App 里打开

导出配合 --json 时须指定 --out，返回文件信息。分析可用 --branch 指定分支。`;

// ───────────────────────── 核心二进制 ─────────────────────────
function corePath(): string {
  if (process.env.CREDITIA_CORE) return process.env.CREDITIA_CORE;
  const exe = process.platform === 'win32' ? 'creditia-core.exe' : 'creditia-core';
  for (const p of [path.join(ROOT, 'target/release', exe), path.join(ROOT, 'target/debug', exe), path.join(HERE, '..', 'bin', `${process.platform}-${process.arch}`, exe)]) if (existsSync(p)) return p;
  return exe;
}
function core(args: string[], inherit = true): { code: number; out: string } {
  const r = spawnSync(corePath(), args, { stdio: inherit ? 'inherit' : ['pipe', 'pipe', 'inherit'], input: args[0] === 'desktop-api' ? '{}' : undefined, encoding: 'utf8' });
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
  return top;
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
  const showPeople = on.has('people') || !!cfg.display?.people;
  raw.people = showPeople ? (cfg.people ?? raw.people ?? []).map((p: any) => ({ ...p, name: cfg.privacy?.aliases?.[p.name] ?? p.name })) : [];
  const m = buildModel(raw, { people: showPeople, lang: (flags.lang as 'zh' | 'en') ?? 'zh' });
  return { m, cfg, raw };
}
function write(out: string | undefined, data: string | Buffer) {
  if (!out) {
    if (json) console.log(JSON.stringify({ content: data.toString() }));
    else process.stdout.write(data);
    return;
  }
  mkdirSync(path.dirname(path.resolve(out)), { recursive: true });
  writeFileSync(out, data);
  if (json) console.log(JSON.stringify({ path: path.resolve(out), bytes: Buffer.byteLength(data) }));
}

// ───────────────────────── 命令 ─────────────────────────
const PASS = new Set(['discover', 'init', 'analyze', 'update', 'collect', 'doctor', 'blame', 'projects', 'stamp', 'hook', 'hook-trailers', 'link', 'people']);

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
      console.log(json ? JSON.stringify({ project: m.project, lines: fetchSegs(m).map(line => line.map(s => s.text).join('')) }) : ansi(fetchSegs(m)));
      return;
    }
    case 'app': {
      const repo = repoOf();
      if (pos[0] && pos[0] !== 'open') die('用法：creditia app open');
      const appArgs = ['--repo', repo, '--form', flags.form ?? 'tier', '--theme', flags.theme ?? 'minimal-light'];
      const target = path.join(ROOT, 'apps/desktop/src-tauri/target/release');
      const candidates = [process.env.CREDITIA_APP, ...(process.platform === 'darwin' ? [path.join(target, 'bundle/macos/creditia.app'), '/Applications/creditia.app', path.join(homedir(), 'Applications/creditia.app')] : []), path.join(target, process.platform === 'win32' ? 'creditia-app.exe' : 'creditia-app')];
      const app = candidates.find(p => p && existsSync(p));
      if (!app) die('没有找到桌面 App。请先构建或安装，并可用 CREDITIA_APP 指定应用完整路径。');
      if (process.platform === 'darwin' && app.endsWith('.app')) {
        const result = spawnSync('open', ['-n', app, '--args', ...appArgs], { stdio: 'pipe', encoding: 'utf8' });
        if (result.status !== 0) die(result.stderr || '启动桌面 App 失败');
      } else {
        await new Promise<void>((resolve, reject) => { const child = spawn(app, appArgs, { detached: true, stdio: 'ignore' }); child.once('error', reject); child.once('spawn', () => { child.unref(); resolve(); }); });
      }
      console.log(json ? JSON.stringify({ path: app, repo, opened: true }) : `已打开 ${repo}`);
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
    console.log(`形式\n${rows.join('\n')}\n\n风格\n${themes.join('\n')}\n\n格式：svg png pdf gif html md json（徽章：--form badge --format svg|md）`);
    return;
  }
  const repo = repoOf();
  const form = flags.form ?? 'tier';
  const theme = themeById(flags.theme ?? 'minimal-light') ?? die(`未知风格：${flags.theme}；creditia export --list 查看`);
  const format = flags.format ?? (form === 'badge' ? 'svg' : 'svg');
  const { m, raw } = modelOf(repo, on.has('refresh'));
  const lang = (flags.lang as 'zh' | 'en') ?? 'zh';
  if (json && !flags.out) die('导出配合 --json 时请指定 --out；结果将返回文件路径');

  if (form === 'badge') {
    if (!['svg', 'md'].includes(format)) die('徽章支持 svg 或 md 格式');
    const svg = badge(m, theme);
    if (format === 'md') {
      const file = flags.out ?? 'assets/creditia-badge.svg';
      write(file, svg);
      if (!json) console.log(`![built with](${file})`);
      return;
    }
    return write(flags.out, svg);
  }
  if (format === 'json') return write(flags.out, JSON.stringify(raw, null, 2) + '\n');
  if (format === 'md') return write(flags.out, creditsBlock(m) + '\n');
  if (!EXPORTS.some(f => f.id === format)) die(`未知格式：${format}；使用 svg、png、pdf、gif、html、md 或 json`);
  if (!FORMS[form]) die(`未知形式：${form}；creditia export --list 查看`);

  // 需要额外数据的形式
  const ctx: Ctx = { lang, variant: flags.variant };
  if (form === 'profile') {
    const r = core(['desktop-api', 'library_credits', '--json'], false);
    if (r.code === 1) die('无法读取作品库');
    ctx.projects = JSON.parse(r.out).projects.map((record: any) => buildModel(record, { people: false, lang }));
    ctx.person = m.signature;
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
    if (!(Number.isFinite(w) && Number.isFinite(h) && w > 0 && h > 0)) die('--size 需要有效的正数尺寸，例如 1200x630');
    svg = await renderSized(form, m, theme, w, h, ctx);
  } else {
    svg = await renderSvg(form, m, theme, ctx);
  }
  if (!svg) die(`${form} 需要的数据不够（例如没有 tag、没有会话记录），没有出图`);
  const width = flags.width ? Number(flags.width) : undefined;
  if (width !== undefined && !(Number.isFinite(width) && width > 0)) die('--width 需要有效的正数');
  const bytes = await encodeExport(svg, format as ExportFormat, svgToRaster, { title: `${m.project} · ${FORMS[form].name}`, form, background: theme.bg, width });
  return write(flags.out, Buffer.from(bytes));
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
