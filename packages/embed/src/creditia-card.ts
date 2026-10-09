// <creditia-card src="credits.json" form="tier" theme="minimal-light" lang="zh" director href="CREDITS.md">
// 读同一份 credits.json，在页面里画 tier。不依赖任何库；logo 从脚本同目录的 logos/ 取（mask 上色）。

type Row = { key: string; name: string; pct: number; lines: number; unknown?: boolean; models: string[] };

const NAMES: Record<string, string> = {
  codex: 'Codex', cursor: 'Cursor', grok: 'Grok', 'claude-code': 'Claude Code', deepseek: 'DeepSeek', kimi: 'Kimi', gemini: 'Gemini', 'github-actions': 'GitHub Actions',
};
const LOGO: Record<string, string> = { codex: 'openai', cursor: 'cursor', grok: 'x', 'claude-code': 'claude', deepseek: 'deepseek', kimi: 'kimi', gemini: 'gemini', 'github-actions': 'github-actions' };
const COLOR: Record<string, [string, string]> = {
  codex: ['#10A37F', '#10A37F'], cursor: ['#0A84FF', '#0A84FF'], grok: ['#26262A', '#E6E6E3'], 'claude-code': ['#D97757', '#D97757'],
  deepseek: ['#4D6BFE', '#4D6BFE'], kimi: ['#1F2329', '#E6E6E3'], gemini: ['#8E75B2', '#8E75B2'], 'github-actions': ['#2088FF', '#2088FF'],
};
const THEMES: Record<string, Record<string, string>> = {
  'minimal-light': { bg: '#FFFFFF', ink: '#161514', soft: '#76726B', faint: '#AEAAA3', line: 'rgba(22,21,20,.08)', track: 'rgba(22,21,20,.06)', edge: 'rgba(22,21,20,.06)', radius: '18px', font: 'Inter, -apple-system, "PingFang SC", "Noto Sans SC", sans-serif', display: 'inherit', weight: '600', dark: '0', sa: '#CFCBC3', sb: '#ECEAE5' },
  'minimal-dark': { bg: '#171716', ink: '#F0EEE9', soft: '#8F8B84', faint: '#5C5953', line: 'rgba(240,238,233,.08)', track: 'rgba(240,238,233,.07)', edge: 'rgba(240,238,233,.06)', radius: '18px', font: 'Inter, -apple-system, "PingFang SC", "Noto Sans SC", sans-serif', display: 'inherit', weight: '600', dark: '1', sa: '#4A4844', sb: '#2A2927' },
  film: { bg: '#12110F', ink: '#EDE6D8', soft: '#8C8577', faint: '#5A554C', line: 'rgba(237,230,216,.1)', track: 'rgba(237,230,216,.08)', edge: 'rgba(237,230,216,.08)', radius: '2px', font: 'Inter, -apple-system, "PingFang SC", sans-serif', display: '"Instrument Serif", "Noto Serif SC", Georgia, serif', weight: '400', dark: '1', sa: '#4D483F', sb: '#26231F' },
  terminal: { bg: '#111418', ink: '#D7DEE5', soft: '#7C8792', faint: '#4B545D', line: 'rgba(215,222,229,.09)', track: 'rgba(215,222,229,.07)', edge: 'rgba(215,222,229,.08)', radius: '6px', font: '"JetBrains Mono", ui-monospace, Menlo, monospace', display: 'inherit', weight: '700', dark: '1', sa: '#3A424A', sb: '#1A1F24' },
  glass: { bg: 'rgba(255,255,255,.58)', ink: '#14161C', soft: '#5E6472', faint: '#9AA0AC', line: 'rgba(20,22,28,.08)', track: 'rgba(20,22,28,.07)', edge: 'rgba(255,255,255,.9)', radius: '28px', font: 'Inter, -apple-system, "PingFang SC", sans-serif', display: 'inherit', weight: '600', dark: '0', sa: '#BFC4CF', sb: '#E4E7EE' },
  editorial: { bg: '#F2EDE3', ink: '#121110', soft: '#6A6359', faint: '#A39B8E', line: '#121110', track: 'rgba(18,17,16,.08)', edge: '#121110', radius: '0', font: 'Inter, -apple-system, "PingFang SC", sans-serif', display: 'Newsreader, "Noto Serif SC", Georgia, serif', weight: '500', dark: '0', sa: '#B5AC9E', sb: '#E2DBCF' },
  mono: { bg: '#FFFFFF', ink: '#000000', soft: '#6B6B6B', faint: '#B0B0B0', line: '#000000', track: '#EDEDED', edge: '#000000', radius: '0', font: 'Inter, -apple-system, "PingFang SC", sans-serif', display: 'inherit', weight: '700', dark: '0', sa: '#C4C4C4', sb: '#FFFFFF' },
};

const fmt = (n: number) => Math.round(n).toLocaleString('en-US');
const pct = (p: number) => (p > 0 && p < 0.1 ? '<0.1%' : `${p.toFixed(1)}%`);

function rows(c: any, lang: string): { rows: Row[]; director: string } {
  const byId = new Map<string, any>((c.contributors ?? []).map((x: any) => [x.id, x]));
  const directors = new Set((c.directors ?? []).map((d: any) => d.id));
  const acc = new Map<string, Row>();
  const modelLines = new Map<string, Map<string, number>>();
  for (const b of c.breakdown ?? []) {
    if (directors.has(b.contributor)) continue;
    const x = byId.get(b.contributor);
    const key = x?.harness ?? (b.contributor === 'unattributed' ? 'unattributed' : b.contributor);
    const e = acc.get(key) ?? { key, name: x?.harness ? NAMES[x.harness] ?? x.harness : x?.kind === 'human' ? x.name ?? key : lang === 'en' ? 'Unattributed' : '未确认', pct: 0, lines: 0, models: [], unknown: key === 'unattributed' };
    e.lines += b.linesAdded ?? 0;
    acc.set(key, e);
    if (x?.harness && x.model) {
      const mm = modelLines.get(key) ?? new Map();
      mm.set(x.model, (mm.get(x.model) ?? 0) + (b.linesAdded ?? 0));
      modelLines.set(key, mm);
    }
  }
  const total = c.totals?.linesAdded || [...acc.values()].reduce((s, x) => s + x.lines, 0);
  for (const e of acc.values()) {
    e.pct = total ? (e.lines / total) * 100 : 0;
    e.models = [...(modelLines.get(e.key) ?? new Map()).entries()].sort((a, b) => b[1] - a[1]).map(([m]) => (m === 'auto' ? 'Auto' : m));
  }
  const unknown = acc.get('unattributed');
  const named = [...acc.values()].filter((x) => !x.unknown).sort((a, b) => b.pct - a.pct);
  const shown = named.filter((x) => x.pct >= 0.1).slice(0, 6);
  const rest = named.filter((x) => !shown.includes(x)).reduce((s, x) => s + x.pct, 0);
  const out = [...shown];
  if (rest >= 0.1) out.push({ key: 'other', name: lang === 'en' ? 'Others' : '其他', pct: rest, lines: 0, models: [] });
  if (unknown && unknown.pct >= 0.05) out.push(unknown);
  return { rows: out, director: c.directors?.[0]?.name ?? '' };
}

class CreditiaCard extends HTMLElement {
  static observedAttributes = ['src', 'theme', 'form', 'lang', 'director', 'href', 'logos'];
  private root = this.attachShadow({ mode: 'open' });
  connectedCallback() { this.render(); }
  attributeChangedCallback() { if (this.isConnected) this.render(); }

  private themeId() {
    const t = this.getAttribute('theme') ?? 'minimal-light';
    if (t === 'auto') return matchMedia('(prefers-color-scheme: dark)').matches ? 'minimal-dark' : 'minimal-light';
    if (t === 'minimal') return 'minimal-light';
    return THEMES[t] ? t : 'minimal-light';
  }

  async render() {
    const src = this.getAttribute('src');
    let data: any = (this as any).credits; // App 等场景可直接赋值，避免走网络
    if (!data && !src) return;
    if (!data) {
      try {
        data = await (await fetch(src!)).json();
      } catch (e) {
        this.root.innerHTML = `<span style="font:13px sans-serif;color:#999">creditia: 读不到 ${src}</span>`;
        return;
      }
    }
    const lang = this.getAttribute('lang') ?? 'zh';
    const t = THEMES[this.themeId()];
    const dark = t.dark === '1';
    const { rows: rs, director } = rows(data, lang);
    const showDirector = this.hasAttribute('director');
    const logos = this.getAttribute('logos') ?? new URL('./logos/', import.meta.url).href;
    const vars = Object.entries(t).map(([k, v]) => `--${k}:${v}`).join(';');
    const row = (r: Row) => {
      const color = r.unknown ? 'transparent' : r.key === 'other' ? t.faint : (COLOR[r.key] ?? ['#999', '#999'])[dark ? 1 : 0];
      const logo = LOGO[r.key];
      const mark = r.unknown
        ? `<span class="mk"><i class="unk"></i></span>`
        : logo ? `<span class="mk logo" style="--m:url('${logos}${logo}.svg')"></span>` : `<span class="mk"></span>`;
      const tip = r.models.length ? `${r.models.slice(0, 3).join(' · ')} · ${fmt(r.lines)} 行` : r.lines ? `${fmt(r.lines)} 行` : '';
      return `<div class="row${r.unknown ? ' unk' : ''}" title="${tip}">
        <div class="l">${mark}<span class="n">${r.name}</span><span class="p">${pct(r.pct)}</span></div>
        <div class="tr"><i style="width:${Math.max(r.pct, 0)}%;${r.unknown ? `background:repeating-linear-gradient(45deg,${t.sa} 0 3px,${t.sb} 3px 6px)` : `background:${color}`}"></i></div>
      </div>`;
    };
    const totals = [data.totals?.commits && `${fmt(data.totals.commits)} ${lang === 'en' ? 'commits' : '次提交'}`, data.totals?.linesAdded && `${fmt(data.totals.linesAdded)} ${lang === 'en' ? 'lines' : '行'}`].filter(Boolean).join('  ·  ');
    const href = this.getAttribute('href');
    const center = this.themeId() === 'film';
    this.root.innerHTML = `<style>
      :host{display:block;max-width:400px;${vars}}
      .card{box-sizing:border-box;padding:28px;background:var(--bg);color:var(--ink);font-family:var(--font);border-radius:var(--radius);border:1px solid var(--edge);${this.themeId() === 'glass' ? 'backdrop-filter:blur(24px);-webkit-backdrop-filter:blur(24px);' : ''}}
      .hd{display:flex;justify-content:space-between;align-items:baseline;margin-bottom:${center ? 24 : 14}px;${center ? 'flex-direction:column;align-items:center;gap:8px;' : ''}}
      .t{font-family:var(--display);font-weight:var(--weight);font-size:${center ? 30 : 18}px;line-height:1}
      .lb{font-size:10px;letter-spacing:.14em;text-transform:uppercase;color:var(--soft)}
      .d{font-size:12px;color:var(--soft);margin:-6px 0 14px}
      .row{padding:9px 0}.row .l{display:flex;align-items:center;gap:10px;font-size:14px;font-weight:500}
      .row.unk .n,.row.unk .p{color:var(--soft)}
      .n{flex:1}.p{font-size:13px;color:var(--soft);font-variant-numeric:tabular-nums}
      .mk{width:16px;height:16px;display:inline-flex;align-items:center;justify-content:center;flex:0 0 auto}
      .logo{background:var(--ink);-webkit-mask:var(--m) center/contain no-repeat;mask:var(--m) center/contain no-repeat}
      .unk{width:10px;height:10px;border-radius:99px;border:1.5px dashed var(--faint)}
      .tr{height:4px;margin-top:9px;border-radius:99px;background:var(--track);overflow:hidden}.tr i{display:block;height:100%;min-width:3px;border-radius:99px;transition:width .6s cubic-bezier(.2,.7,.2,1)}
      .ft{margin-top:14px;padding-top:14px;border-top:1px solid var(--line);font-size:12px;color:var(--soft);display:flex;justify-content:space-between;${center ? 'justify-content:center;' : ''}}
      a{color:inherit;text-decoration:none;border-bottom:1px solid var(--line)}
      @media (prefers-reduced-motion: reduce){.tr i{transition:none}}
    </style>
    <div class="card" part="card">
      <div class="hd"><span class="t">${data.project?.name ?? ''}</span><span class="lb">Credits</span></div>
      ${showDirector && director ? `<div class="d">${director}</div>` : ''}
      ${rs.map(row).join('')}
      <div class="ft"><span>${totals}</span>${href ? `<a href="${href}">CREDITS.md</a>` : ''}</div>
    </div>`;
    // 条形从零长出
    requestAnimationFrame(() => {
      this.root.querySelectorAll<HTMLElement>('.tr i').forEach((el) => {
        const w = el.style.width;
        el.style.width = '0';
        requestAnimationFrame(() => (el.style.width = w));
      });
    });
  }
}
if (!customElements.get('creditia-card')) customElements.define('creditia-card', CreditiaCard);
export {};
