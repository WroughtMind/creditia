// 第 2 段：catnap 的代码地图。10 个真文件、真行数、各工具真份额；141 天里一段段写出来，有的段落被别的工具改写
// 坐标：地图自己的平面（x 向右，y 向上＝往文件开头，z 离开地面）。原点 = pick.ts 第一行的左端
import * as THREE from 'three';
import { rng, clamp, lerp } from '../short/lib.js';
import { CW, LP, textCanvas, textMesh, tex } from './text.js';

export const KEYS = ['claude-code', 'codex', 'cursor', 'kimi', 'grok'];
export const UN = 5, HUMAN = 6; // 未确认（灰）、人写的（不上色，也不出现在任何名字里）
export const CAP = 300, COLC = 46, COLP = COLC * CW + 6.2, FILEGAP = 3.5;

export function buildMap({ data, C, colors, heroLines }) {
  const group = new THREE.Group();
  const r = rng(101);
  const days = data.days, ND = days.length;
  const cls = (k) => (KEYS.includes(k) ? KEYS.indexOf(k) : k === 'unattributed' ? UN : HUMAN);
  const active = (c, d) => c >= UN || (days[Math.min(ND - 1, Math.floor(d))].by[KEYS[c]] ?? 0) > 0;

  // ── 每一行：属于哪个文件、哪一段、谁写的、哪天写的 ──
  const lines = []; // { file, col, row, cls, d0, c0, d1, shade, hero }
  let colX = 0, col = 0;
  const cols = [];
  data.files.forEach((f, fi) => {
    const cnt = [0, 0, 0, 0, 0, 0, 0];
    for (const [k, v] of Object.entries(f.by)) cnt[cls(k)] += v;
    const seq = [];
    if (fi === 0) for (const h of heroLines) { seq.push({ c: h.cls, hero: true, text: h.text }); cnt[h.cls] = Math.max(0, cnt[h.cls] - 1); }
    // 其余按剩下的份额切成一段段
    let rest = cnt.reduce((a, b) => a + b, 0);
    const blocks = [];
    while (rest > 0) {
      let v = r() * rest, c = 0; while (v > cnt[c]) { v -= cnt[c]; c++; }
      const n = Math.min(cnt[c], 3 + Math.floor(r() * 26));
      blocks.push({ c, n }); cnt[c] -= n; rest -= n;
    }
    // 哪天写：每个文件有自己的起止，段落大体从上往下写；工具那天要真的在干活
    const s = fi === 0 ? 0 : Math.floor(Math.pow(r(), 1.6) * 48), e = s + (ND - 1 - s) * (0.62 + r() * 0.38);
    const heroDay = 2;
    blocks.forEach((b, j) => {
      let d = s + (e - s) * Math.pow((j + r() * 0.8) / blocks.length, 0.9);
      let guard = 0; while (!active(b.c, d) && d < ND - 1 && guard++ < 40) d += 1;
      b.d = Math.min(ND - 1, d);
      // 两成多的段落先被别的工具写过一稿，后来改写
      b.d0 = b.d; b.c0 = b.c;
      if (b.c < UN && r() < 0.24) { const c0 = Math.floor(r() * 5); const d0 = Math.max(s, b.d - 3 - r() * 18); if (c0 !== b.c && active(c0, d0)) { b.c0 = c0; b.d0 = d0; } }
    });
    if (fi === 0) seq.forEach((x) => { x.d = heroDay + seq.indexOf(x) * 0.04; x.d0 = x.d; x.c0 = x.c; });
    for (const b of blocks) for (let i = 0; i < b.n; i++) seq.push({ c: b.c, d: b.d + i * 0.03, d0: b.d0 + i * 0.03, c0: b.c0 });
    // 排版：每列最多 CAP 行，一个文件写不下就接着下一列
    seq.forEach((x, i) => {
      if (i > 0 && i % CAP === 0) { col++; colX += COLP; }
      if (i === 0 && fi > 0) { col++; colX += COLP + FILEGAP; }
      lines.push({ file: fi, col, x0: colX, row: i % CAP, ...x });
      if (i % CAP === 0) cols.push({ file: fi, x: colX });
    });
  });
  const mapW = colX + COLC * CW;

  // ── 一行拆成几笔（一笔＝一个词）；pick.ts 开头那几行按真代码拆，近看时和真字对得上 ──
  const strokes = [];
  let ind = 0;
  for (const L of lines) {
    const y = -L.row * LP;
    if (L.hero) {
      if (!L.text.trim()) continue;
      for (const m of L.text.matchAll(/\S+/g)) strokes.push([L.x0 + m.index * CW, y, m[0].length * CW, L.d0, L.c0, L.c, L.d, 0.8, 1]);
      continue;
    }
    if (r() < 0.11) continue; // 空行
    ind = L.row === 0 ? 0 : Math.max(0, Math.min(4, ind + (r() < 0.2 ? 1 : r() < 0.25 ? -1 : 0)));
    let x = L.x0 + ind * 2 * CW; const end = x + Math.max(4, (COLC - ind * 2) * (0.18 + r() * 0.72)) * CW;
    let first = true;
    while (x < end - CW) { const w = Math.min(end - x, (2 + Math.floor(r() * 9)) * CW); strokes.push([x, y, w, L.d0, L.c0, L.c, L.d, first ? 0.62 : r() < 0.5 ? 0.92 : 0.76, 0]); x += w + CW; first = false; }
  }

  const quad = new THREE.PlaneGeometry(1, 1);
  const geo = new THREE.InstancedBufferGeometry();
  geo.index = quad.index; geo.setAttribute('position', quad.getAttribute('position'));
  geo.setAttribute('a0', new THREE.InstancedBufferAttribute(new Float32Array(strokes.flatMap((s) => [s[0], s[1], s[2], s[3]])), 4));
  geo.setAttribute('a1', new THREE.InstancedBufferAttribute(new Float32Array(strokes.flatMap((s) => [s[4], s[5], s[6], s[7]])), 4));
  geo.setAttribute('a2', new THREE.InstancedBufferAttribute(new Float32Array(strokes.map((s) => s[8])), 1));
  geo.instanceCount = strokes.length;
  const pal = colors.map((c) => C(c));
  const mat = new THREE.ShaderMaterial({
    uniforms: { day: { value: -1 }, alpha: { value: 1 }, heroA: { value: 1 }, pal: { value: pal }, flash: { value: 1 } },
    vertexShader: `attribute vec4 a0, a1; attribute float a2; uniform float day, heroA, flash; uniform vec3 pal[7]; varying vec3 vC; varying float vA;
      void main(){
        float k = clamp((day - a0.w) / .7, 0., 1.);
        float rewritten = step(a1.z, day);
        int c = int(rewritten > .5 ? a1.y : a1.x);
        vec3 col = pal[0];
        for (int i = 0; i < 7; i++) if (i == c) col = pal[i];
        // 刚写下、刚被改写的那一刻亮一下
        float since = rewritten > .5 ? day - a1.z : day - a0.w;
        float hot = exp(-max(since, 0.) * 2.2) * step(0., since) * flash;
        vC = col * (1. + 1.6 * hot);
        vA = a1.w * step(.0001, k) * mix(1., heroA, a2);
        vec3 p = vec3(a0.x + (position.x + .5) * a0.z * k, a0.y + position.y * .46, 0.);
        gl_Position = projectionMatrix * modelViewMatrix * vec4(p, 1.);
      }`,
    fragmentShader: `uniform float alpha; varying vec3 vC; varying float vA; void main(){ gl_FragColor = vec4(vC, vA * alpha); if (gl_FragColor.a < .003) discard; }`,
    transparent: true, depthWrite: false,
  });
  const mesh = new THREE.Mesh(geo, mat); mesh.frustumCulled = false; mesh.renderOrder = 2;
  group.add(mesh);

  // ── 底下一条时间轴：日期在走，提交一颗颗落下，到了发版那天标出版本号 ──
  const t0 = Date.parse(days[0].date), t1 = Date.parse(days[ND - 1].date);
  const span = Math.round((t1 - t0) / 864e5); // 140（含首尾 141 天）
  const calOf = (d) => { const i = Math.floor(clamp(d, 0, ND - 1)), j = Math.min(ND - 1, i + 1), f = clamp(d, 0, ND - 1) - i; return (Date.parse(days[i].date) - t0 + (Date.parse(days[j].date) - Date.parse(days[i].date)) * f) / 864e5; };
  const dayOfCal = (c) => { let i = 0; while (i < ND - 1 && (Date.parse(days[i + 1].date) - t0) / 864e5 <= c) i++; const a = (Date.parse(days[i].date) - t0) / 864e5, b = i < ND - 1 ? (Date.parse(days[i + 1].date) - t0) / 864e5 : a + 1; return i + clamp((c - a) / Math.max(1, b - a)); };
  // 时间轴在地图的远端（文件开头那一边）：镜头斜着往前看时它在画面上方
  const RY = 15, xOfCal = (c) => (c / span) * mapW;
  const ink = C('#E9E2D3');
  const ruler = new THREE.Mesh(new THREE.PlaneGeometry(1, 0.22), new THREE.MeshBasicMaterial({ color: ink, transparent: true, opacity: 0, depthWrite: false }));
  ruler.geometry.translate(0.5, 0, 0); ruler.position.set(0, RY, 0); group.add(ruler);
  // 提交：按每天写的量分配 734 个
  const tot = days.map((d) => Object.values(d.by).reduce((a, b) => a + b, 0)), sum = tot.reduce((a, b) => a + b, 0);
  const ticks = [];
  let acc = 0, made = 0;
  days.forEach((d, i) => {
    acc += (tot[i] / sum) * data.totals.commits;
    const n = Math.round(acc) - made; made += n;
    const ent = Object.entries(d.by), all = ent.reduce((a, [, v]) => a + v, 0);
    for (let k = 0; k < n; k++) {
      let v = r() * all, c = UN; for (const [key, x] of ent) { v -= x; if (v <= 0) { c = cls(key); break; } }
      const dd = i + (k / Math.max(1, n)) * 0.9;
      ticks.push([xOfCal(calOf(dd)), RY + 1.2, 1.2 + r() * 3.2, dd, c]);
    }
  });
  const tgeo = new THREE.InstancedBufferGeometry();
  tgeo.index = quad.index; tgeo.setAttribute('position', quad.getAttribute('position'));
  tgeo.setAttribute('b0', new THREE.InstancedBufferAttribute(new Float32Array(ticks.flatMap((s) => [s[0], s[1], s[2], s[3]])), 4));
  tgeo.setAttribute('b1', new THREE.InstancedBufferAttribute(new Float32Array(ticks.map((s) => s[4])), 1));
  tgeo.instanceCount = ticks.length;
  const tmat = new THREE.ShaderMaterial({
    uniforms: { day: { value: -1 }, alpha: { value: 1 }, pal: { value: pal } },
    vertexShader: `attribute vec4 b0; attribute float b1; uniform float day; uniform vec3 pal[7]; varying vec3 vC; varying float vA;
      void main(){
        float k = clamp((day - b0.w) / 1.2, 0., 1.);
        vec3 col = pal[0]; for (int i = 0; i < 7; i++) if (i == int(b1)) col = pal[i];
        vC = col * (1. + 2. * (1. - k)); vA = step(.0001, k);
        float fall = (1. - k) * (1. - k) * 26.;                 // 从半空落到时间轴上
        vec3 p = vec3(b0.x + position.x * .32, b0.y + (position.y + .5) * b0.z, fall);
        gl_Position = projectionMatrix * modelViewMatrix * vec4(p, 1.);
      }`,
    fragmentShader: `uniform float alpha; varying vec3 vC; varying float vA; void main(){ gl_FragColor = vec4(vC, vA * alpha * .9); if (gl_FragColor.a < .003) discard; }`,
    transparent: true, depthWrite: false,
  });
  const tmesh = new THREE.Mesh(tgeo, tmat); tmesh.frustumCulled = false; tmesh.renderOrder = 3; group.add(tmesh);
  // 版本号
  const vers = data.versions.map((v) => {
    const c = (Date.parse(v.date) - t0) / 864e5;
    const m = textMesh(textCanvas([{ text: v.tag, color: '#E9E2D3', family: 'Mono' }], 96, { pad: 0.1 }), 4.2, 0.5);
    m.position.set(xOfCal(c), RY - 4.6, 0.02); group.add(m);
    const mk = new THREE.Mesh(new THREE.PlaneGeometry(0.3, 7), new THREE.MeshBasicMaterial({ color: ink, transparent: true, opacity: 0, depthWrite: false }));
    mk.position.set(xOfCal(c), RY + 1.4, 0.01); group.add(mk);
    return { m, mk, d: dayOfCal(c) };
  });
  // 走动的日期
  const dcv = document.createElement('canvas'); dcv.width = 720; dcv.height = 160;
  const dtex = tex(dcv);
  const dw = 15, dmesh = new THREE.Mesh(new THREE.PlaneGeometry(dw, (dw * 160) / 720), new THREE.MeshBasicMaterial({ map: dtex, transparent: true, depthWrite: false, opacity: 0 }));
  dmesh.renderOrder = 21; group.add(dmesh);
  const cursor = new THREE.Mesh(new THREE.PlaneGeometry(0.22, 9), new THREE.MeshBasicMaterial({ color: ink, transparent: true, opacity: 0, depthWrite: false }));
  group.add(cursor);
  let lastDate = '';
  const fmt = (ms) => new Date(ms).toISOString().slice(0, 10);

  function update(day, { alpha = 1, heroA = 1, ui = 1 } = {}) {
    mat.uniforms.day.value = day; mat.uniforms.alpha.value = alpha; mat.uniforms.heroA.value = heroA;
    tmat.uniforms.day.value = day; tmat.uniforms.alpha.value = alpha * ui;
    const c = calOf(day), x = xOfCal(c);
    ruler.material.opacity = 0.3 * ui * alpha; ruler.scale.x = Math.max(0.001, mapW);
    for (const v of vers) { const k = clamp((day - v.d) / 1.5); v.m.material.opacity = k * ui * alpha * 0.9; v.mk.material.opacity = k * ui * alpha * 0.6; }
    const s = fmt(t0 + Math.min(span, Math.max(0, c)) * 864e5);
    if (s !== lastDate) {
      lastDate = s; const g = dcv.getContext('2d'); g.clearRect(0, 0, dcv.width, dcv.height);
      g.font = '500 112px Mono'; g.fillStyle = '#E9E2D3'; g.textBaseline = 'middle'; g.textAlign = 'center'; g.fillText(s, dcv.width / 2, dcv.height * 0.52);
      dtex.needsUpdate = true;
    }
    dmesh.material.opacity = ui * alpha; dmesh.position.set(x, RY + 12, 0.03);
    cursor.material.opacity = 0.7 * ui * alpha; cursor.position.set(x, RY + 2.6, 0.02);
  }
  return { group, update, mapW, cols, strokes: strokes.length, ticks: ticks.length, RY, span };
}
