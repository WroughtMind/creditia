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

/** 模型名一行：最多三个。按模型分组时这一行本身就是模型，写它用的工具 */
export const modelLine = (a: Agent, n = 3) => a.model ? a.toolName : a.models.slice(0, n).map((x) => x.name).join(' · ') + (a.models.length > n ? ` +${a.models.length - n}` : '');

/** 人做了什么：只写数字。v 是数字，k 是放在数字下面的短标签，text 是放进一句话里的说法 */
export function handFacts(m: Model): { v: string; k: string; text: string }[] {
  // 署名只写 AI：人这一侧的数字不进作品
  if (!m.showHand) return [];
  const h = m.hand;
  const out: { v: string; k: string; text: string }[] = [];
  if (h.prompts) out.push({ v: fmt(h.prompts), k: T(m, '条消息发给 AI', 'messages to AI'), text: T(m, `发了 ${fmt(h.prompts)} 条消息`, `${fmt(h.prompts)} messages sent`) });
  if (h.reviews) out.push({ v: fmt(h.reviews), k: T(m, '次 review', 'reviews'), text: T(m, `review ${fmt(h.reviews)} 次`, `${fmt(h.reviews)} reviews`) });
  if (h.handLines) out.push({ v: fmt(h.handLines), k: T(m, '行是人写的', 'lines by people'), text: T(m, `人写了 ${fmt(h.handLines)} 行`, `${fmt(h.handLines)} lines by people`) });
  return out;
}
export function handLine(m: Model): string {
  return handFacts(m).map((f) => f.text).join(' · ');
}
/** 署名：用户主动填写时；named 时列出人名 */
export function peopleNames(m: Model, n = 4): string {
  if (!m.named || !m.people.length) return '';
  const names = m.people.slice(0, n).map((p) => p.name);
  return names.join(zh(m) ? '、' : ', ') + (m.people.length > n ? T(m, ` 等 ${m.people.length} 人`, ` +${m.people.length - n}`) : '');
}
export const handTip = (m: Model) => ({ k: 'human', t: T(m, '人', 'People'), d: [
  ...handFacts(m).map((f) => f.text),
  ...(m.hand.written && m.hand.committed ? [T(m, `AI 写了 ${fmt(m.hand.written)} 行，提交了 ${fmt(m.hand.committed)} 行，现在还在 ${fmt(m.hand.surviving)} 行`, `${fmt(m.hand.written)} written by AI → ${fmt(m.hand.committed)} committed → ${fmt(m.hand.surviving)} still here`)] : []),
  ...(peopleNames(m) ? [peopleNames(m)] : []),
] });

export const range = (m: Model) => (m.range.from ? `${m.range.from.replaceAll('-', '.')} — ${m.range.to.replaceAll('-', '.')}` : '');
export const summary = (m: Model) => T(m, `${fmt(m.commits)} 次提交 · ${fmt(m.linesAdded)} 行${m.spanDays ? ` · ${m.spanDays} 天` : ''}`, `${fmt(m.commits)} commits · ${fmt(m.linesAdded)} lines${m.spanDays ? ` · ${m.spanDays} days` : ''}`);
export { fmt, compact, nameOf };

/** 画布：底图铺满，卡片面避开风格的保留边距。innerH 是内容区高度。
 *  fixed：画幅是定死的（分享图 1200×630 这类平台尺寸），底板不加高，内容区照旧收进框线里 */
export function canvas(m: Model, st: Style, W: number, innerH: number, M = 34, fixed = false): { s: Scene; c: Box; H: number } {
  const R = st.reserve;
  // 底板自己有框线和留白：先在草稿上量出它往里收多少，底板加高这么多，内容区才真有 innerH 那么高
  const probe = fixed ? null : st.plate(new Scene(W, innerH + 2000), { x: M, y: M + R.top, w: W - M * 2, h: innerH + 1000 });
  const inset = !probe ? 0 : Math.max(0, probe.y - (M + R.top)) + Math.max(0, M + R.top + innerH + 1000 - (probe.y + probe.h));
  const H = innerH + inset + M * 2 + R.top + R.bottom;
  const s = new Scene(W, H);
  st.ground(s, { x: 0, y: 0, w: W, h: H }, m);
  const c = st.plate(s, { x: M, y: M + R.top, w: W - M * 2, h: innerH + inset });
  return { s, c, H };
}

/** 底板角上装饰往内容区里伸多深（没有就是 0）：在草稿上画一次底板量出来 */
export const plateCorner = (st: Style, W: number, M = 34) => st.plate(new Scene(W, 2000), { x: M, y: M, w: W - M * 2, h: 1000 }).corner ?? 0;

/** 竖排文字的高度（与 Scene.vtext 同一算法） */
export function measureV(str: string, st: Style, size: number, font = st.f.display): number {
  let h = 0;
  for (const run of str.match(/[\u2E80-\u9FFF\uF900-\uFAFF\uFF00-\uFFEF\u3000-\u303F]|[^\u2E80-\u9FFF\uF900-\uFAFF\uFF00-\uFFEF\u3000-\u303F]+/g) ?? []) {
    if (/[\u2E80-\u9FFF\uF900-\uFAFF\uFF00-\uFFEF\u3000-\u303F]/.test(run)) h += size * 1.08;
    else h += measure(run.trim(), font, size * 0.92) + size * 0.3;
  }
  return h;
}

/** 排序位置：按工具分组时就是排名；按模型分组时先按工具的排名、再按工具内的排名，同一工具的模型挨在一起。没上榜的排最后 */
export function keyRank(m: Model, k: string): number {
  const i = m.rows.findIndex((x) => x.key === k);
  if (m.group !== 'model') return i + 1 || 500;
  const ti = m.tools.findIndex((x) => x.key === k.split('/')[0]);
  return ti < 0 ? (i + 1 || 50) * 1000 : ti * 50 + (i + 1 || 49);
}
/** 图例的顺序：同上 */
export const legendRows = (m: Model) => [...m.rows].sort((a, b) => keyRank(m, a.key) - keyRank(m, b.key));
