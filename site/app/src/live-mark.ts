// 活的星号 *：App 的状态都由它来表达。写法照官网首屏那颗（site/index.html 的 liveMark）：
// 每瓣的长度、透明度、方向各是一个带阻尼的弹簧，颜色在 OKLCH 里过渡，整颗再有一个转角的弹簧。
// 四种样子：静止（项目自己的份额）、读取（一道呼吸沿着瓣转）、进度（灰的瓣一瓣瓣长满）、出错（收短、变灰、抖两下）。
// 只在有东西在动时跑 rAF；系统要求减少动态效果时，一律直接定格。
import { markPaths, MARK_COLORS, MARK_GREY, type Petal } from '../../../packages/render/src/mark.ts';

export type MarkMode = 'idle' | 'loading' | 'progress' | 'error';
const RM = matchMedia('(prefers-reduced-motion: reduce)');

// ── 颜色：hex ⇄ OKLCH，过渡时不经过脏灰 ──
type Lch = [number, number, number];
const lin = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const gam = (c: number) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);
function toLch(hex: string): Lch {
  if (!/^#[0-9a-f]{6}$/i.test(hex)) hex = MARK_GREY; // currentColor 之类：当灰色
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((x) => lin(x / 255));
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s;
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return [L, Math.hypot(A, B), (Math.atan2(B, A) * 180) / Math.PI];
}
function toHex([L, C, h]: Lch): string {
  const A = C * Math.cos((h * Math.PI) / 180), B = C * Math.sin((h * Math.PI) / 180);
  const l = (L + 0.3963377774 * A + 0.2158037573 * B) ** 3;
  const m = (L - 0.1055613458 * A - 0.0638541728 * B) ** 3;
  const s = (L - 0.0894841775 * A - 1.291485548 * B) ** 3;
  const rgb = [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s];
  return '#' + rgb.map((c) => Math.round(Math.min(1, Math.max(0, gam(c))) * 255).toString(16).padStart(2, '0')).join('');
}
function mixLch(a: Lch, b: Lch, t: number): Lch {
  // 灰（几乎没有彩度）的色相没有意义：跟着有颜色的那头走，免得半路转一圈彩虹
  const ha = a[1] < 0.02 ? b[2] : a[2], hb = b[1] < 0.02 ? a[2] : b[2];
  let dh = hb - ha; if (dh > 180) dh -= 360; if (dh < -180) dh += 360;
  return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, ha + dh * t];
}

// ── 弹簧 ──
type Spring = { x: number; v: number; to: number };
const sp = (x: number): Spring => ({ x, v: 0, to: x });
/** 推一步；还在动就返回 true */
function step(s: Spring, k: number, c: number, dt: number) {
  const a = k * (s.to - s.x) - c * s.v;
  s.v += a * dt; s.x += s.v * dt;
  return Math.abs(s.v) > 1e-3 || Math.abs(s.to - s.x) > 1e-3;
}
const snap = (s: Spring) => { s.x = s.to; s.v = 0; };

type Slot = { len: Spring; op: Spring; at: Spring; mix: Spring; from: Lch; to: Lch; color: string };

const live = new Set<LiveMark>();
let raf = 0, last = 0;
function wake() { if (!raf) { last = performance.now(); raf = requestAnimationFrame(loop); } }
function loop(now: number) {
  const dt = Math.min(1 / 30, (now - last) / 1000); last = now;
  let busy = false;
  // 暂时不在页面上（换位置的间隙、所在的那块被重画）就先不动，挂回去时 fly / poke 会再叫醒
  for (const m of live) if (m.el.isConnected && m.tick(dt, now / 1000)) busy = true;
  raf = busy ? requestAnimationFrame(loop) : 0;
}

/** 品牌的 *：五瓣五色，全长 */
export const BRAND: Petal[] = MARK_COLORS.map((color) => ({ color }));
/** 还没分析过：五瓣淡淡的短瓣 */
export const GHOST: Petal[] = Array.from({ length: 5 }, () => ({ color: MARK_GREY, len: 0.5, opacity: 0.35 }));

export class LiveMark {
  /** 外层负责位置（飞行），里面的 svg 负责转角 */
  readonly el: HTMLSpanElement;
  private svg: SVGSVGElement;
  private slots: Slot[] = [];
  private shape: Petal[] = BRAND;
  private m: MarkMode = 'idle';
  private p: number | null = 0;
  private th = sp(0);
  private t0 = performance.now() / 1000;

  constructor(private o: { optical?: boolean; label?: string } = {}) {
    this.el = document.createElement('span');
    this.el.className = 'lm';
    this.el.innerHTML = `<svg class="mk" viewBox="0 0 24 24" aria-hidden="true"></svg>`;
    this.svg = this.el.firstElementChild as SVGSVGElement;
    live.add(this);
  }

  get mode() { return this.m; }
  get petals() { return this.shape; }

  /** 换成另一组瓣（一个项目的份额）。grow：从中心长出来；turn：同时转几瓣 */
  set(petals: Petal[], o: { grow?: boolean; turn?: number } = {}) {
    this.shape = petals.length ? petals : GHOST;
    if (o.grow) for (const s of this.slots) { s.len.x = 0; s.len.v = 0; s.op.x = 0; }
    if (o.turn) this.turn(o.turn);
    this.retarget();
  }
  setMode(m: MarkMode) {
    if (m === this.m) return;
    if (this.m === 'progress') this.th.to = Math.ceil(this.th.to / 72 - 0.01) * 72; // 转着等的那段停下时，落在整瓣上
    this.m = m;
    if (m === 'loading' || m === 'progress') this.t0 = performance.now() / 1000;
    this.el.classList.toggle('err', m === 'error');
    if (m === 'error' && !RM.matches) { this.el.classList.remove('shake'); void this.el.offsetWidth; this.el.classList.add('shake'); }
    this.retarget();
  }
  /** 0–1；null 是不确定（没有总数），这时慢慢转着呼吸 */
  progress(p: number | null) {
    this.p = p == null ? null : Math.max(this.p ?? 0, Math.min(1, p)); // 只增不减
    if (p === 0) this.p = 0;
    this.retarget();
  }
  turn(n: number) { this.th.to += 72 * n; if (RM.matches) snap(this.th); wake(); }
  /** 某一瓣轻跳一下 */
  kick(i = 0) { if (RM.matches) return; const s = this.slots[i]; if (s) { s.len.v += 6; wake(); } }

  private retarget() {
    const shape = this.m === 'progress' || this.m === 'error' ? null : this.shape;
    const n = shape ? shape.length : 5;
    while (this.slots.length < n) {
      const c = toLch(MARK_GREY);
      this.slots.push({ len: sp(0), op: sp(0), at: sp(this.slots.length / n), mix: sp(1), from: c, to: c, color: MARK_GREY });
    }
    this.slots.forEach((s, i) => {
      let len = 0, op = 0, color = MARK_GREY;
      if (shape) {
        const p = shape[i];
        if (p) { len = p.len ?? 1; op = p.opacity ?? 1; color = p.color; }
      } else if (i < 5) {
        if (this.m === 'error') { len = 0.5; op = 0.55; }
        else { const f = this.p == null ? 0.5 : Math.min(1, Math.max(0, this.p * 5 - i)); len = 0.36 + 0.64 * f; op = 0.32 + 0.68 * f; }
      }
      s.len.to = len; s.op.to = op;
      s.at.to = i < n ? i / n : i / this.slots.length;
      if (color !== s.color) { s.from = this.lch(s); s.to = toLch(color); s.color = color; s.mix.x = 0; s.mix.v = 0; s.mix.to = 1; }
      if (RM.matches) { snap(s.len); snap(s.op); snap(s.at); snap(s.mix); }
    });
    wake();
  }
  private lch(s: Slot): Lch { return mixLch(s.from, s.to, Math.min(1, Math.max(0, s.mix.x))); }

  /** 推一帧；还在动就返回 true */
  tick(dt: number, now: number): boolean {
    const t = now - this.t0, rm = RM.matches;
    // 读取：一道呼吸沿着瓣转。进度不确定：一边呼吸一边慢慢转
    const breathe = !rm && (this.m === 'loading' || (this.m === 'progress' && this.p == null));
    if (!rm && this.m === 'progress' && this.p == null) this.th.to += dt * 40;
    let moving = breathe;
    const n = this.m === 'progress' || this.m === 'error' ? 5 : this.shape.length;
    const k = this.m === 'progress' ? 120 : 210, c = this.m === 'progress' ? 20 : 13;
    this.slots.forEach((s, i) => {
      const base = s.len.to;
      if (breathe && i < n) s.len.to = base * (1 + 0.16 * Math.sin((2 * Math.PI * t) / 1.3 - (i * 2 * Math.PI) / n));
      moving = step(s.len, k, c, dt) || moving;
      s.len.to = base;
      moving = step(s.op, 170, 26, dt) || moving;
      moving = step(s.at, 120, 22, dt) || moving;
      moving = step(s.mix, 90, 19, dt) || moving;
    });
    moving = step(this.th, 70, 10, dt) || moving;
    if (!moving) this.slots.length = n;
    this.draw();
    return moving;
  }
  private draw() {
    // 与 SVG 的透明度精度一致：已经不可见的旧瓣不再占据扇区宽度。
    const petals: Petal[] = this.slots.map((s) => ({ color: toHex(this.lch(s)), len: Math.max(0, s.len.x), opacity: +Math.max(0, Math.min(1, s.op.x)).toFixed(3), at: s.at.x })).filter((p) => p.opacity > 0);
    this.svg.innerHTML = markPaths({ petals, optical: this.o.optical });
    this.svg.style.transform = `rotate(${this.th.x.toFixed(2)}deg)`;
  }
  /** 立刻画出当前目标（不等动画），比如刚挂到页面上时 */
  now() { for (const s of this.slots) { snap(s.len); snap(s.op); snap(s.at); snap(s.mix); } snap(this.th); this.draw(); }
  /** 挂回页面之后接着动 */
  poke() { wake(); }
  /** 不用了：退出动画循环 */
  dispose() { live.delete(this); }
}

/** FLIP：把 el 挪进 to，看上去是从原来的位置、原来的大小飞过去的 */
export function fly(el: HTMLElement, to: HTMLElement, ms = 500) {
  if (el.parentElement === to) return;
  const a = el.isConnected ? el.getBoundingClientRect() : null;
  to.appendChild(el);
  wake();
  const b = el.getBoundingClientRect();
  if (!a || !a.width || !b.width || RM.matches) return;
  const dx = a.left - b.left, dy = a.top - b.top, s = a.width / b.width;
  if (Math.abs(dx) < 1 && Math.abs(dy) < 1 && Math.abs(s - 1) < 0.01) return;
  el.animate([{ transformOrigin: '0 0', transform: `translate(${dx}px, ${dy}px) scale(${s})` }, { transformOrigin: '0 0', transform: 'none' }], { duration: ms, easing: 'cubic-bezier(.2,.8,.2,1)' });
}
