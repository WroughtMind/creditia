// 第 1 段：终端。打 creditia analyze，出三行结果；再打 creditia fetch，出那张名片（同 fetchSegs 的真输出）
// 坐标：终端自己的平面，第一行左端在 (0, 0)，往下一行 y 减 1
import * as THREE from 'three';
import { rng, clamp, prog } from '../short/lib.js';
import { CW, LP, monoLine } from './text.js';

const INK = '#E9E2D3', DIM = '#7D776C', PROMPT = '~/catnap $ ';

export function buildTerminal({ data, C, T }) {
  const group = new THREE.Group();
  const r = rng(3);
  const rows = []; // { segs, t0, t1, typed: [start, len] }
  // 人敲命令：每个字之间的停顿不一样，词和词之间多停一下
  const typing = (cmd, t0) => {
    const times = []; let t = t0;
    for (const ch of cmd) { times.push(t); t += 0.045 + r() * 0.05 + (ch === ' ' ? 0.09 : 0); }
    return { times, end: t };
  };
  const a = T.term;
  const c1 = typing('npx creditia analyze', a.type1);
  rows.push({ segs: [{ text: PROMPT, color: DIM }, { text: 'npx creditia analyze', color: INK }], show: a.start, typed: c1, plen: PROMPT.length });
  data.analyze.forEach((s, i) => rows.push({ segs: [{ text: s, color: i === 1 ? '#B9B1A2' : INK }], show: a.out1 + i * 0.07 }));
  rows.push({ segs: [], show: a.out1 });
  const c2 = typing('npx creditia fetch', a.type2);
  rows.push({ segs: [{ text: PROMPT, color: DIM }, { text: 'npx creditia fetch', color: INK }], show: a.prompt2, typed: c2, plen: PROMPT.length });
  const fetchRow0 = rows.length;
  data.fetch.forEach((ln, i) => rows.push({ segs: ln.map((s) => ({ text: s.text, color: s.color ?? (s.dim ? DIM : INK), bold: s.bold })), show: a.out2 + i * 0.045 }));
  rows.push({ segs: [{ text: PROMPT, color: DIM }], show: a.out2 + data.fetch.length * 0.045 + 0.1, plen: PROMPT.length });

  for (const [i, R] of rows.entries()) {
    if (!R.segs.length) continue;
    R.m = monoLine(R.segs); R.m.position.set(0, -i * LP, 0); group.add(R.m);
  }
  // 方块光标：敲字时常亮，等着的时候闪
  const cur = new THREE.Mesh(new THREE.PlaneGeometry(CW * 0.92, LP * 0.78), new THREE.MeshBasicMaterial({ color: C(INK), transparent: true, depthWrite: false }));
  cur.position.z = 0.01; group.add(cur);

  // 钻进去的那一点：Claude Code 那一格进度条的中间
  const barRow = fetchRow0 + 2, barCol = 16 + 6.5;
  const target = new THREE.Vector3(barCol * CW, -barRow * LP, 0);

  function update(t, { alpha = 1 } = {}) {
    let last = -1, typingNow = false;
    rows.forEach((R, i) => {
      if (t >= R.show) last = i;
      if (!R.m) return;
      let rev = t >= R.show ? R.m.userData.len + 1 : 0;
      if (R.typed && t >= R.show) {
        rev = R.plen + R.typed.times.filter((x) => x <= t).length;
        if (t >= R.typed.times[0] - 0.05 && t < R.typed.end + 0.15) typingNow = true;
      }
      R.m.material.uniforms.rev.value = rev; R.m.material.uniforms.alpha.value = alpha;
    });
    // 光标：在提示符后面等着、跟着敲的字走；回车后落到下一行
    let cx = 0, cy = 0;
    const R = rows[last];
    if (R && R.plen != null && !(R.typed && t > R.typed.end + 0.25)) { cy = last; cx = R.plen + (R.typed ? R.typed.times.filter((x) => x <= t).length : 0); }
    else if (R) { cy = last + 1; cx = 0; }
    const blink = typingNow ? 1 : (Math.floor(t * 1.9) % 2 === 0 ? 1 : 0);
    cur.material.opacity = (last >= 0 ? blink : 0) * alpha * 0.85;
    cur.position.set(cx * CW + CW * 0.5, -cy * LP, 0.01);
  }
  // 给配乐：每一下按键、每一行输出出现的时刻
  const keys = [...c1.times, ...c2.times], lines = rows.filter((R) => !R.typed && R.segs.length).map((R) => R.show);
  return { group, update, target, rows: rows.length, keys, lines, enters: [c1.end + 0.25, c2.end + 0.25] };
}
