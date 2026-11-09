import { r, type Scene } from '../svg.ts';
import type { Style, Box } from '../style.ts';
import { materialEnter, materialReveal } from '../material-motion.ts';
import { drawGroup } from '../motion.ts';

/** 量条的长度/高度是真值：材质沿厚度形成，或整条印入，不能沿数据轴截短。 */
export function valueBar(s: Scene, st: Style, k: number, b: Pick<Box, 'x' | 'y' | 'w' | 'h'>, axis: 'x' | 'y', draw: () => void): void {
  k = Math.max(0, Math.min(1, k));
  if (k >= 1) { draw(); return; }
  if (['editorial', 'mono', 'cinema', 'playbill'].includes(st.id)) {
    materialEnter(s, st, k, b, draw, 'data', axis);
    return;
  }
  if (st.id === 'poster' || st.id === 'terminal') {
    materialEnter(s, st, k, b, draw, 'data', axis);
    return;
  }
  if (st.id === 'ukiyoe') {
    // 整张真实色版轻微对位；位移只沿厚度，不改变任何值轴的起止位置。
    const offset = r(-2 * (1 - k)), first = s.hits.length;
    drawGroup(s, { transform: axis === 'x' ? `translate(0 ${offset})` : `translate(${offset} 0)` }, () => materialEnter(s, st, k, b, draw, 'data', axis));
    for (let i = first; i < s.hits.length; i++) s.hits[i][axis === 'x' ? 'y' : 'x'] = r(s.hits[i][axis === 'x' ? 'y' : 'x'] + offset);
    return;
  }
  // 舞台窗、聚光与字符扫描具有自身固定方向，不适合拿来截取真实份额。
  const whole = st.cells || ['terminal', 'arcade', 'cyber', 'baroque', 'playbill', 'gala', 'woodblock'].includes(st.id)
    || axis === 'y' && ['editorial', 'crt'].includes(st.id);
  if (whole) materialEnter(s, st, k, b, draw, 'data', axis);
  else materialReveal(s, st, k, b, axis === 'x' ? 'down' : 'right', draw);
}
