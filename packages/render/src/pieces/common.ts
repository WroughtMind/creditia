// 作品共用：参数、文案、人的部分。
import type { Model, Agent } from '../data.ts';
import { fmt, compact, tipOf, nameOf } from '../data.ts';
import type { Style } from '../style.ts';
import { Scene, measure } from '../svg.ts';
import type { Box } from '../style.ts';

export interface PieceOpts {
  variant?: string;
  /** 动画进度 0→1；静态导出用 1 */
  t?: number;
}
export type Piece = (m: Model, st: Style, o: PieceOpts) => Scene | null;

export const zh = (m: Model) => m.lang === 'zh';
export const T = (m: Model, z: string, e: string) => (m.lang === 'zh' ? z : e);

/** 排名里的贡献方（不含「人」） */
export const ranked = (m: Model) => m.rows;
export const colorOf = (st: Style, a: Agent) => st.color(a.key, a.rank);
export const fillOf = (s: Scene, st: Style, a: Agent) => (a.key === 'unattributed' ? st.hatch(s) : colorOf(st, a));
export const tip = tipOf;

/** 模型名一行：最多三个 */
export const modelLine = (a: Agent, n = 3) => a.models.slice(0, n).map((x) => x.name).join(' · ') + (a.models.length > n ? ` +${a.models.length - n}` : '');

/** 人的部分：用做了什么来写，不写头衔 */
export function handFacts(m: Model): { v: string; k: string }[] {
  const h = m.hand;
  const out: { v: string; k: string }[] = [];
  if (h.prompts) out.push({ v: fmt(h.prompts), k: T(m, '条指令', 'prompts') });
  if (h.reviews) out.push({ v: fmt(h.reviews), k: T(m, '次审阅', 'reviews') });
  if (h.handLines) out.push({ v: fmt(h.handLines), k: T(m, '行亲手写下', 'lines by hand') });
  if (h.written > h.committed && h.committed) out.push({ v: `${Math.round((h.committed / h.written) * 100)}%`, k: T(m, '的生成被采纳', 'of generated code kept') });
  return out;
}
export function handLine(m: Model): string {
  return handFacts(m).map((f) => (zh(m) ? `${f.v} ${f.k}` : `${f.v} ${f.k}`)).join(zh(m) ? ' · ' : ' · ');
}
/** 署名：用户主动填写时；named 时列出人名 */
export function peopleNames(m: Model, n = 4): string {
  if (!m.named || !m.people.length) return '';
  const names = m.people.slice(0, n).map((p) => p.name);
  return names.join(zh(m) ? '、' : ', ') + (m.people.length > n ? T(m, ` 等 ${m.people.length} 人`, ` +${m.people.length - n}`) : '');
}
export const handTip = (m: Model) => ({ k: 'human', t: T(m, '人的部分', 'The human hand'), d: [
  ...handFacts(m).map((f) => `${f.v} ${f.k}`),
  ...(m.hand.written && m.hand.committed ? [T(m, `agent 写下 ${fmt(m.hand.written)} 行，提交 ${fmt(m.hand.committed)} 行，至今留下 ${fmt(m.hand.surviving)} 行`, `${fmt(m.hand.written)} generated → ${fmt(m.hand.committed)} committed → ${fmt(m.hand.surviving)} remain`)] : []),
  ...(peopleNames(m) ? [peopleNames(m)] : []),
] });

export const range = (m: Model) => (m.range.from ? `${m.range.from.replaceAll('-', '.')} — ${m.range.to.replaceAll('-', '.')}` : '');
export const summary = (m: Model) => T(m, `${fmt(m.commits)} 次提交 · ${fmt(m.linesAdded)} 行${m.spanDays ? ` · ${m.spanDays} 天` : ''}`, `${fmt(m.commits)} commits · ${fmt(m.linesAdded)} lines${m.spanDays ? ` · ${m.spanDays} days` : ''}`);
export { fmt, compact, nameOf };

/** 画布：底图铺满，卡片面避开风格的保留边距。innerH 是内容区高度 */
export function canvas(m: Model, st: Style, W: number, innerH: number, M = 34): { s: Scene; c: Box; H: number } {
  const R = st.reserve;
  const H = innerH + M * 2 + R.top + R.bottom;
  const s = new Scene(W, H);
  st.ground(s, { x: 0, y: 0, w: W, h: H }, m);
  const c = st.plate(s, { x: M, y: M + R.top, w: W - M * 2, h: innerH });
  return { s, c, H };
}

/** 竖排文字的高度（与 Scene.vtext 同一算法） */
export function measureV(str: string, st: Style, size: number, font = st.f.display): number {
  let h = 0;
  for (const run of str.match(/[\u2E80-\u9FFF\uF900-\uFAFF\uFF00-\uFFEF\u3000-\u303F]|[^\u2E80-\u9FFF\uF900-\uFAFF\uFF00-\uFFEF\u3000-\u303F]+/g) ?? []) {
    if (/[\u2E80-\u9FFF\uF900-\uFAFF\uFF00-\uFFEF\u3000-\u303F]/.test(run)) h += size * 1.08;
    else h += measure(run.trim(), font, size * 0.92) + size * 0.3;
  }
  return h;
}
