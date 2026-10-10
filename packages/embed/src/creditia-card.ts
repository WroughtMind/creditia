// <creditia-card src="credits.json" form="tier" theme="minimal-light" lang="zh" director href="CREDITS.md" logos="./logos/">
// 读同一份 credits.json，在页面里画 tier。六种风格家族六种排版，和导出图同一套设计语言。不依赖任何库；
// logo 从脚本同目录的 logos/ 取（mask 上色）。
import { buildModel, fmt, pct, type Model } from '../../render/src/data.ts';
import { themeById, type Theme } from '../../render/src/themes.ts';
import { famColor } from '../../render/src/brand.ts';

const LOGO: Record<string, string> = { codex: 'openai', cursor: 'cursor', grok: 'x', 'claude-code': 'claude', deepseek: 'deepseek', kimi: 'kimi', gemini: 'gemini', 'github-actions': 'github-actions' };

const CSS = `
:host{display:block;max-width:400px}
.card{box-sizing:border-box;padding:26px 28px;background:var(--bg);color:var(--ink);font-family:var(--sans);border-radius:var(--radius);border:1px solid var(--edge)}
.card.glass{background:var(--surface);backdrop-filter:blur(24px);-webkit-backdrop-filter:blur(24px);box-shadow:0 20px 60px rgba(40,48,80,.12);border-color:rgba(255,255,255,.9)}
.card.film{border-color:var(--edge);text-align:center}
.card.term,.card.mono{padding:0;overflow:hidden;border-radius:6px}
.hd{display:flex;justify-content:space-between;align-items:baseline;margin-bottom:12px}
.t{font-family:var(--display);font-weight:var(--weight);font-size:18px;line-height:1}
.lb{font-size:10px;letter-spacing:.14em;text-transform:uppercase;color:var(--soft)}
.d{font-size:12px;color:var(--soft);margin:-6px 0 10px}
.row{padding:8px 0}.row .l{display:flex;align-items:center;gap:10px;font-size:14px;font-weight:500}
.n{flex:1}.p{font-size:13px;color:var(--soft);font-variant-numeric:tabular-nums}
.mk{width:16px;height:16px;display:inline-flex;align-items:center;justify-content:center;flex:0 0 auto}
.logo{-webkit-mask:var(--m) center/contain no-repeat;mask:var(--m) center/contain no-repeat}
.unk{width:10px;height:10px;border-radius:99px;border:1.5px dashed var(--faint)}
.tr{height:4px;margin-top:8px;border-radius:99px;background:var(--track);overflow:hidden}.tr i{display:block;height:100%;min-width:3px;border-radius:99px;transition:width .6s cubic-bezier(.2,.7,.2,1)}
.hum{margin-top:10px;font-size:12px;color:var(--soft)}.hum .lb{margin-right:8px}
.ft{margin-top:14px;padding-top:12px;border-top:1px solid var(--line);font-size:12px;color:var(--soft);display:flex;justify-content:space-between}
a{color:inherit;text-decoration:none;border-bottom:1px solid var(--line)}
/* film */
.film .fhead{display:flex;flex-direction:column;align-items:center;gap:9px;margin-bottom:14px}
.film .flab{font-size:10px;letter-spacing:.3em;text-transform:uppercase;color:var(--accent)}
.film .ftitle{font-family:var(--display);font-weight:var(--weight);font-size:33px;line-height:1.05}
.film .fdir{font-family:var(--display);font-size:14px;color:var(--soft)}
.film .fdots{color:var(--accent);letter-spacing:.5em;font-size:11px}
.frow{display:flex;align-items:baseline;justify-content:center;gap:11px;padding:7px 0}
.film .fname{font-family:var(--display);font-weight:var(--weight);font-size:25px}
.fp{font-size:12px;color:var(--faint);font-variant-numeric:tabular-nums}
.fhum{font-family:var(--display);font-size:14px;color:var(--soft);margin-top:12px}
.film .ft{justify-content:center;font-size:10px;letter-spacing:.2em;text-transform:uppercase}
/* terminal */
.tbar{display:flex;align-items:center;gap:8px;padding:9px 16px;border-bottom:1px solid var(--edge);font:13px/1 var(--sans)}
.tprompt{color:var(--accent)}.tproj{margin-left:auto;color:var(--faint);font-size:12px}
.tbody{padding:13px 18px 16px;font:13px/1.9 var(--sans)}
.tnote{color:var(--faint);font-size:11.5px;margin:2px 0}
.trow{display:flex;align-items:center;gap:10px}
.tconn{color:var(--faint)}.tn{flex:0 0 auto;min-width:96px}
.tb{flex:1;text-align:right;letter-spacing:.06em;font-size:12px}.tp{width:52px;text-align:right;color:var(--soft);font-size:12px;font-variant-numeric:tabular-nums}
.term .ft{margin:8px 18px 14px;border-top:0;padding-top:4px;color:var(--faint);font-size:11.5px}
/* glass */
.gpill{margin-bottom:8px;padding:10px 14px;border-radius:18px;background:rgba(255,255,255,.66);border:1px solid rgba(255,255,255,.9);box-shadow:0 1px 2px rgba(20,22,28,.05),0 6px 16px rgba(20,22,28,.05)}
.grow{display:flex;align-items:center;gap:10px}
.gchip{width:26px;height:26px;border-radius:99px;display:inline-flex;align-items:center;justify-content:center;flex:0 0 auto}
.gn{flex:1;font-size:14px;font-weight:600}.gp{font-size:14px;font-weight:700;font-variant-numeric:tabular-nums}
.gbar{height:4px;margin-top:7px;border-radius:99px;background:var(--track);overflow:hidden}.gbar i{display:block;height:100%;min-width:3px;border-radius:99px;transition:width .6s cubic-bezier(.2,.7,.2,1)}
/* editorial */
.ed{padding:20px 24px;border:2px solid var(--edge)}
.ehead{display:flex;justify-content:space-between;align-items:baseline}
.etitle{font-family:var(--display);font-weight:var(--weight);font-size:34px;line-height:1}
.erule{height:3px;background:var(--ink);margin:10px 0 4px}
.erow{display:flex;align-items:baseline;gap:0;padding:10px 0;border-bottom:1px solid rgba(18,17,16,.28)}
.en{font-size:16px;font-weight:500}
.eleader{flex:1;margin:0 10px;height:2px;background-image:repeating-linear-gradient(90deg,var(--faint) 0 2px,transparent 2px 7px)}
.ep{font-family:var(--display);font-size:22px;font-variant-numeric:tabular-nums}
.ehum{margin-top:10px;font-size:12.5px;color:var(--soft)}.ehum .lb{margin-right:8px}
.erule2{height:2px;background:var(--ink);margin-top:8px}
.ed .ft{border-top:0;padding-top:8px}
/* mono */
.mhead{display:flex;justify-content:space-between;align-items:center;background:var(--ink);color:var(--bg);padding:11px 16px;font-weight:700;font-size:14px;letter-spacing:.04em;text-transform:uppercase}
.mhead span{font-weight:400;font-size:10px;letter-spacing:.2em;opacity:.72;text-transform:uppercase}
.mdir{padding:7px 16px 0;font-size:12px;color:var(--soft)}
.mbody{padding:6px 16px 10px}
.mrow{display:flex;align-items:center;gap:12px;padding:10px 0;border-bottom:1px solid rgba(0,0,0,.18)}
.mb{width:52px;height:26px;display:inline-flex;align-items:center;justify-content:center;color:#fff;font-size:11.5px;font-weight:700;font-variant-numeric:tabular-nums;flex:0 0 auto}
.mn{flex:1;font-weight:700;font-size:13.5px;letter-spacing:.04em;text-transform:uppercase}
.mhum{display:flex;align-items:center;gap:8px;margin-top:10px;font-size:10.5px;letter-spacing:.08em;text-transform:uppercase;color:var(--soft)}
.mhum i{width:8px;height:8px;background:var(--ink)}
.mfoot{background:var(--ink);color:var(--bg);padding:11px 16px;font-size:11px;letter-spacing:.1em;text-transform:uppercase}
@media (prefers-reduced-motion: reduce){.tr i,.gbar i{transition:none}}
`;

class CreditiaCard extends HTMLElement {
  static observedAttributes = ['src', 'theme', 'form', 'lang', 'director', 'href', 'logos'];
  private root = this.attachShadow({ mode: 'open' });
  connectedCallback() { this.render(); }
  attributeChangedCallback() { if (this.isConnected) this.render(); }

  private themeId() {
    const t = this.getAttribute('theme') ?? 'minimal-light';
    if (t === 'auto') return matchMedia('(prefers-color-scheme: dark)').matches ? 'minimal-dark' : 'minimal-light';
    return themeById(t) ? t : 'minimal-light';
  }

  private mark(k: string, t: Theme, logos: string, color?: string): string {
    if (k === 'unattributed') return `<span class="mk"><i class="unk"></i></span>`;
    const logo = LOGO[k];
    if (!logo) return `<span class="mk"></span>`;
    return `<span class="mk logo" style="--m:url('${logos}${logo}.svg');background:${color ?? t.ink}"></span>`;
  }

  async render() {
    const src = this.getAttribute('src');
    let data: any = (this as any).credits; // App 等场景可直接赋值，避免走网络
    if (!data && !src) return;
    if (!data) {
      try { data = await (await fetch(src!)).json(); }
      catch { this.root.innerHTML = `<span style="font:13px sans-serif;color:#999">creditia: 读不到 ${src}</span>`; return; }
    }
    const lang = (this.getAttribute('lang') ?? 'zh') as 'zh' | 'en';
    const t = themeById(this.themeId())!;
    const m = buildModel(data, { director: this.hasAttribute('director'), lang });
    const logos = this.getAttribute('logos') ?? new URL('./logos/', import.meta.url).href;
    const vars = `--bg:${t.bg};--surface:${t.surface};--ink:${t.ink};--soft:${t.soft};--faint:${t.faint};--line:${t.line};--track:${t.track};--edge:${t.edge};--radius:${t.radius}px;--accent:${t.accent};--sans:${t.family === 'terminal' ? '"JetBrains Mono",ui-monospace,Menlo,monospace' : 'Inter,-apple-system,"PingFang SC","Noto Sans SC",sans-serif'};--display:${t.family === 'film' ? '"Instrument Serif",Georgia,serif' : t.family === 'editorial' ? 'Newsreader,Georgia,serif' : 'inherit'};--weight:${t.displayWeight};--sa:${t.stripeA};--sb:${t.stripeB}`;
    this.root.innerHTML = `<style>${CSS}</style>${this.tierHtml(m, t, logos, lang)}`;
    const card = this.root.querySelector('.card') as HTMLElement | null;
    if (card) card.setAttribute('style', vars);
    requestAnimationFrame(() => {
      this.root.querySelectorAll<HTMLElement>('[data-w]').forEach((el) => {
        const w = el.dataset.w;
        if (!w) return;
        el.style.width = '0';
        requestAnimationFrame(() => (el.style.width = w));
      });
    });
  }

  private tierHtml(m: Model, t: Theme, logos: string, lang: 'zh' | 'en'): string {
    const rows = m.rows.filter((r) => r.key !== 'human');
    const totals = [m.commits && `${fmt(m.commits)} ${lang === 'en' ? 'commits' : '次提交'}`, m.linesAdded && `${fmt(m.linesAdded)} ${lang === 'en' ? 'lines' : '行'}`].filter(Boolean).join('  ·  ');
    const href = this.getAttribute('href');
    const foot = `<div class="ft"><span>${totals}</span>${href ? `<a href="${href}">CREDITS.md</a>` : ''}</div>`;
    const hum = m.humans.length ? m.humans.slice(0, 4).map((x) => `${x.name} ${pct(x.pct)}`).join('　') : '';
    const stripe = `repeating-linear-gradient(45deg,var(--sa) 0 3px,var(--sb) 3px 6px)`;
    switch (t.family) {
      case 'film':
        return `<div class="card film"><div class="fhead"><span class="flab">Credits</span><div class="ftitle">${m.project}</div>${m.director ? `<div class="fdir">由 ${m.director} 主导</div>` : ''}<div class="fdots">· · ·</div></div>${rows.map((r) => `<div class="frow">${this.mark(r.key, t, logos)}<span class="fname">${r.name}</span><span class="fp">${pct(r.pct)}</span></div>`).join('')}${hum ? `<div class="fhum">— 人 · ${hum} —</div>` : ''}${foot}</div>`;
      case 'terminal':
        return `<div class="card term"><div class="tbar"><span class="tprompt">❯</span> creditia cred <span class="tproj">${m.project}</span></div><div class="tbody">${m.director ? `<div class="tnote"># 由 ${m.director} 主导</div>` : ''}${rows.map((r, i) => { const k = Math.max(r.pct >= 0.05 ? 1 : 0, Math.round((r.pct / 100) * 9)); const c = r.key === 'unattributed' ? 'var(--faint)' : famColor(r.key, t); return `<div class="trow"><span class="tconn">${i === rows.length - 1 ? '└─' : '├─'}</span>${this.mark(r.key, t, logos)}<span class="tn">${r.name}</span><span class="tb"><i style="color:${c}">${'■'.repeat(k)}</i><i style="color:var(--faint)">${'·'.repeat(9 - k)}</i></span><span class="tp">${pct(r.pct)}</span></div>`; }).join('')}${hum ? `<div class="tnote"># 人写的部分：${hum}</div>` : ''}${foot}</div></div>`;
      case 'glass':
        return `<div class="card glass"><div class="hd"><span class="t">${m.project}</span><span class="lb">Credits</span></div>${m.director ? `<div class="d">${m.director}</div>` : ''}${rows.map((r) => { const c = r.key === 'unattributed' ? t.faint : famColor(r.key, t); return `<div class="gpill"><div class="grow"><span class="gchip" style="background:${c}22">${this.mark(r.key, t, logos, c)}</span><span class="gn">${r.name}</span><span class="gp">${pct(r.pct)}</span></div><div class="gbar"><i data-w="${Math.max(r.pct, 0)}%" style="background:${r.key === 'unattributed' ? stripe : c}"></i></div></div>`; }).join('')}${hum ? `<div class="hum">人 · ${hum}</div>` : ''}${foot}</div>`;
      case 'editorial':
        return `<div class="card ed"><div class="ehead"><span class="etitle">${m.project}</span><span class="lb">Credits</span></div><div class="erule"></div>${rows.map((r) => `<div class="erow"><span class="en">${r.name}</span><span class="eleader"></span><span class="ep">${pct(r.pct)}</span></div>`).join('')}${hum ? `<div class="ehum"><span class="lb">人</span>${hum}</div>` : ''}<div class="erule2"></div>${foot}</div>`;
      case 'mono':
        return `<div class="card mono"><div class="mhead">${m.project}<span>Credits</span></div>${m.director ? `<div class="mdir">${m.director}</div>` : ''}<div class="mbody">${rows.map((r) => `<div class="mrow"><span class="mb" style="background:${r.key === 'unattributed' ? 'var(--faint)' : 'var(--ink)'}">${pct(r.pct)}</span><span class="mn">${r.name}</span>${this.mark(r.key, t, logos)}</div>`).join('')}${hum ? `<div class="mhum"><i></i>人 · ${hum}</div>` : ''}</div><div class="mfoot">${totals}</div></div>`;
      default:
        return `<div class="card"><div class="hd"><span class="t">${m.project}</span><span class="lb">Credits</span></div>${m.director ? `<div class="d">${m.director}</div>` : ''}${rows.map((r) => { const c = r.key === 'unattributed' ? t.faint : famColor(r.key, t); return `<div class="row"><div class="l">${this.mark(r.key, t, logos)}<span class="n">${r.name}</span><span class="p">${pct(r.pct)}</span></div><div class="tr"><i data-w="${Math.max(r.pct, 0)}%" style="background:${r.key === 'unattributed' ? stripe : c}"></i></div></div>`; }).join('')}${hum ? `<div class="hum"><span class="lb">人</span>${hum}</div>` : ''}${foot}</div>`;
    }
  }
}
if (!customElements.get('creditia-card')) customElements.define('creditia-card', CreditiaCard);
export {};
