// Browser embedding shares the desktop artwork, layout and animation requests.
import { styleById } from '../../render/src/styles/index.ts';
import { INTERACT_CSS, mountHits } from '../../render/src/interact.ts';
import type { Art } from '../../render/src/export.ts';
import { timeAt, type Animation } from '../../render/src/timeline.ts';
import { renderWeb } from './backend.ts';

class CreditiaCard extends HTMLElement {
  static observedAttributes = ['src', 'piece', 'form', 'theme', 'renderer', 'variant', 'lang', 'group', 'named', 'sign', 'animated'];
  private raw: unknown;
  private generation = 0;
  private loading: AbortController | null = null;
  private off: (() => void) | null = null;
  private animation: Animation | null = null;
  private elapsed = 0;
  private lastTick = 0;
  private raf = 0;
  private index = -1;
  private paused = false;
  private visible = true;
  private observer: IntersectionObserver | null = null;
  private media = matchMedia('(prefers-reduced-motion: reduce)');
  private artRoot: HTMLDivElement | null = null;
  private control: HTMLButtonElement | null = null;
  private activity = () => { this.lastTick = 0; this.schedule(); };
  private reduced = () => {
    this.lastTick = 0;
    if (this.media.matches && this.animation?.frames.length) this.show(this.animation.frames.at(-1)!);
    else this.schedule();
  };
  connectedCallback() {
    this.observer = new IntersectionObserver(entries => {
      this.visible = entries[0]?.isIntersecting ?? false; this.activity();
    });
    this.observer.observe(this);
    document.addEventListener('visibilitychange', this.activity);
    this.media.addEventListener('change', this.reduced);
    void this.load();
  }
  disconnectedCallback() {
    this.generation++; this.loading?.abort(); this.stop(); this.off?.();
    this.observer?.disconnect(); this.observer = null;
    document.removeEventListener('visibilitychange', this.activity);
    this.media.removeEventListener('change', this.reduced);
  }
  attributeChangedCallback(name: string, previous: string | null, next: string | null) {
    if (!this.isConnected || previous === next) return;
    if (name === 'src') void this.load(); else void this.paint();
  }
  private language() { return this.getAttribute('lang') === 'en' ? 'en' : 'zh'; }
  private selection() {
    const style = this.getAttribute('theme') || 'minimal', group = this.getAttribute('group');
    return {
      raw: this.raw, style, renderer: this.getAttribute('renderer') || (styleById(style) ? 'svg' : 'classic'),
      piece: this.getAttribute('piece') || this.getAttribute('form') || 'tier',
      ...(this.getAttribute('variant') ? { variant: this.getAttribute('variant') } : {}),
      lang: this.language(), group: group === 'tool' || group === 'model' ? group : 'auto',
      named: this.hasAttribute('named') && this.getAttribute('named') !== 'false', sign: this.getAttribute('sign') || '',
    };
  }
  private async load() {
    const version = ++this.generation;
    this.loading?.abort(); this.stop(); this.raw = null;
    const abort = this.loading = new AbortController(); this.setAttribute('aria-busy', 'true');
    try {
      const response = await fetch(this.getAttribute('src') || '/.creditia/credits.json', { signal: abort.signal });
      if (!response.ok) throw new Error(`Archive: HTTP ${response.status}`);
      const bytes = await response.arrayBuffer();
      if (bytes.byteLength > 8 * 1024 * 1024) throw new Error('Archive exceeds 8 MiB');
      const raw = JSON.parse(new TextDecoder().decode(bytes));
      if (!raw || typeof raw !== 'object' || Array.isArray(raw)) throw new Error('Invalid credits archive');
      if (version !== this.generation || !this.isConnected) return;
      this.raw = raw; await this.paint();
    } catch (error) { if (version === this.generation && !abort.signal.aborted) this.fail(error); }
  }
  private async paint() {
    if (!this.raw || !this.isConnected) return;
    const version = ++this.generation;
    this.stop(); this.animation = null; this.elapsed = 0; this.index = -1;
    this.setAttribute('aria-busy', 'true'); const request = this.selection();
    try {
      const result = await renderWeb({ ...request, op: 'render' });
      if (version !== this.generation || !this.isConnected) return;
      if (!result.art) {
        this.textContent = this.language() === 'zh' ? '这份档案没有该作品所需的数据' : 'This archive has no data for this artwork';
        this.setAttribute('aria-busy', 'false'); return;
      }
      if (!document.getElementById('creditia-interact')) {
        const css = document.createElement('style'); css.id = 'creditia-interact';
        css.textContent = INTERACT_CSS + '\ncreditia-card{display:block}creditia-card>.ct-play{margin-top:8px;font:inherit;padding:4px 12px;cursor:pointer}';
        document.head.appendChild(css);
      }
      this.off?.(); this.replaceChildren();
      this.artRoot = document.createElement('div'); this.artRoot.className = 'ct-art';
      this.artRoot.style.width = '100%'; this.appendChild(this.artRoot);
      this.control = null; this.show(result.art); this.setAttribute('aria-busy', 'false');
      this.dispatchEvent(new CustomEvent('creditia-ready', { detail: { art: result.art } }));
      if (this.hasAttribute('animated')) {
        try {
          const sequence = await renderWeb({ ...request, op: 'frames', frameCount: 48, reducedMotion: this.media.matches });
          if (version !== this.generation || !this.isConnected) return;
          this.animation = sequence; this.paused = false;
          this.control = document.createElement('button'); this.control.className = 'ct-play'; this.control.type = 'button';
          this.control.onclick = () => this.paused ? this.play() : this.pause();
          this.appendChild(this.control); this.updateControl(); this.schedule();
        } catch (error) { this.dispatchEvent(new CustomEvent('creditia-animation-error', { detail: error })); }
      }
    } catch (error) { if (version === this.generation) this.fail(error); }
  }
  private show(art: Art) {
    if (!this.artRoot) return;
    this.off?.(); this.artRoot.style.maxWidth = `${art.w}px`; this.artRoot.innerHTML = art.svg;
    this.artRoot.setAttribute('role', 'img'); this.artRoot.setAttribute('aria-label', art.title);
    this.off = mountHits(this.artRoot, art.hits, art.w, art.h);
  }
  private fail(error: unknown) {
    this.off?.(); this.off = null; this.setAttribute('aria-busy', 'false'); this.setAttribute('role', 'status');
    this.textContent = this.language() === 'zh' ? '作品加载失败' : 'Unable to load artwork';
    this.dispatchEvent(new CustomEvent('creditia-error', { detail: error }));
  }
  private stop() { cancelAnimationFrame(this.raf); this.raf = 0; this.lastTick = 0; }
  private schedule() {
    if (this.raf || !this.animation || this.paused || !this.visible || document.hidden || this.media.matches || !this.isConnected) return;
    this.raf = requestAnimationFrame(now => {
      this.raf = 0;
      if (!this.animation || this.paused || !this.visible || document.hidden || this.media.matches) { this.lastTick = 0; return; }
      if (this.lastTick) this.elapsed += Math.max(0, now - this.lastTick);
      this.lastTick = now; const time = timeAt(this.elapsed, this.animation.timeline); let index = 0;
      for (let i = 0; i < this.animation.times.length; i++) if (this.animation.times[i] <= time) index = i;
      if (index !== this.index && this.animation.frames[index]) { this.index = index; this.show(this.animation.frames[index]); }
      if (!this.animation.timeline.loop && time >= 1) this.pause(); else this.schedule();
    });
  }
  private updateControl() {
    if (!this.control) return;
    this.control.textContent = this.paused ? (this.language() === 'zh' ? '播放' : 'Play') : (this.language() === 'zh' ? '暂停' : 'Pause');
    this.control.setAttribute('aria-pressed', String(!this.paused));
  }
  play() { this.paused = false; this.lastTick = 0; this.updateControl(); this.schedule(); }
  pause() { this.paused = true; this.stop(); this.updateControl(); }
  replay() { this.elapsed = 0; this.index = -1; this.play(); }
}
if (!customElements.get('creditia-card')) customElements.define('creditia-card', CreditiaCard);
