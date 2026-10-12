// <creditia-card src="/.creditia/credits.json" piece="tier" theme="ink" variant="vertical" lang="zh" named sign="Lin">
// 读同一份 credits.json，在页面里画出和导出图一模一样的作品（同一套 SVG 场景），悬停能看细节。
// 字体按风格从 Google Fonts 取；不依赖任何框架。
import { buildModel } from '../../render/src/data.ts';
import { styleById } from '../../render/src/styles/index.ts';
import { art } from '../../render/src/registry.ts';
import { INTERACT_CSS, mountHits } from '../../render/src/interact.ts';
import { FONTS, familiesIn } from '../../render/src/fonts.ts';

const loaded = new Set<string>();
function loadFonts(families: string[]) {
  const want = families.filter((f) => !loaded.has(f) && FONTS.some((x) => x.family === f));
  if (!want.length) return;
  want.forEach((f) => loaded.add(f));
  const q = want.map((f) => {
    const faces = FONTS.filter((x) => x.family === f);
    const ital = faces.some((x) => x.italic);
    const ws = [...new Set(faces.map((x) => x.weight))].sort((a, b) => a - b);
    const axes = ital ? `:ital,wght@${[...ws.map((w) => `0,${w}`), ...faces.filter((x) => x.italic).map((x) => `1,${x.weight}`)].join(';')}` : ws.length > 1 || ws[0] !== 400 ? `:wght@${ws.join(';')}` : '';
    return `family=${f.replace(/ /g, '+')}${axes}`;
  }).join('&');
  const link = document.createElement('link');
  link.rel = 'stylesheet';
  link.href = `https://fonts.googleapis.com/css2?${q}&display=swap`;
  document.head.appendChild(link);
}

class CreditiaCard extends HTMLElement {
  static observedAttributes = ['src', 'piece', 'theme', 'variant', 'lang', 'named', 'sign'];
  private raw: any = null;
  private off: (() => void) | null = null;
  connectedCallback() { this.load(); }
  attributeChangedCallback(name: string) { if (name === 'src') this.load(); else this.paint(); }
  private async load() {
    const src = this.getAttribute('src') ?? '/.creditia/credits.json';
    try { this.raw = await (await fetch(src)).json(); } catch { this.textContent = ''; return; }
    this.paint();
  }
  private async paint() {
    if (!this.raw) return;
    const st = styleById(this.getAttribute('theme') ?? 'minimal') ?? styleById('minimal')!;
    const m = buildModel(this.raw, { lang: (this.getAttribute('lang') as 'zh' | 'en') ?? 'zh', named: this.hasAttribute('named'), sign: this.getAttribute('sign') ?? '' });
    const a = art(this.getAttribute('piece') ?? this.getAttribute('form') ?? 'tier', m, st, { variant: this.getAttribute('variant') ?? undefined });
    if (!a) return;
    loadFonts([st.f.display, st.f.body, st.f.num, st.f.label, st.f.mono, st.f.script].filter(Boolean).flatMap((f) => familiesIn(f!.family)));
    if (!document.getElementById('creditia-interact')) {
      const s = document.createElement('style');
      s.id = 'creditia-interact';
      s.textContent = INTERACT_CSS;
      document.head.appendChild(s);
    }
    this.off?.();
    this.innerHTML = `<div class="ct-art" style="width:100%;max-width:${a.w}px">${a.svg}</div>`;
    this.off = mountHits(this.firstElementChild as HTMLElement, a.hits, a.w, a.h);
  }
}
if (!customElements.get('creditia-card')) customElements.define('creditia-card', CreditiaCard);
