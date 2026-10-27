// 第 3 段：证据。pick.ts 开头 16 行变回真字，每一行行尾标出它凭什么算在谁头上：
//   日志里对得上的（实心星号，一根线连到那个工具会话记录里同样的一行）
//   提交里写明的（半透明星号，一根线连到提交信息里的 Creditia-* trailer）
//   推断的（很淡的星号，没有线）；未确认的（灰色虚线框，没有星号）
// 坐标：地图坐标（见 map.js），面板浮在地面上方
import * as THREE from 'three';
import { clamp, prog, smr, eo3, esin, lerp } from '../short/lib.js';
import { CW, LP, monoLine, codeSegs, tex, starCanvas } from './text.js';
import { HERO } from './evidence-data.js';

const INK = '#E9E2D3', DIM = '#7D776C';

function roundRect(g, x, y, w, h, r) { g.beginPath(); g.moveTo(x + r, y); g.arcTo(x + w, y, x + w, y + h, r); g.arcTo(x + w, y + h, x, y + h, r); g.arcTo(x, y + h, x, y, r); g.arcTo(x, y, x + w, y, r); g.closePath(); }

// 面板：一块圆角的暗色底，上面几行等宽字；返回每一行的左端位置（面板坐标）
function panel({ rows, C }) {
  const g = new THREE.Group();
  const s = 0.62; // 面板里的字比代码小一点
  const width = Math.max(...rows.map((r) => r.reduce((n, x) => n + x.text.length, 0))) * CW * s + 1.8;
  const h = rows.length * LP * s + 1.6;
  const cv = document.createElement('canvas'); cv.width = 512; cv.height = Math.round((512 * h) / width);
  const x = cv.getContext('2d'); x.fillStyle = 'rgba(28,26,23,0.92)'; roundRect(x, 2, 2, cv.width - 4, cv.height - 4, 22); x.fill();
  x.strokeStyle = 'rgba(233,226,211,0.10)'; x.lineWidth = 2; roundRect(x, 2, 2, cv.width - 4, cv.height - 4, 22); x.stroke();
  const bg = new THREE.Mesh(new THREE.PlaneGeometry(width, h), new THREE.MeshBasicMaterial({ map: tex(cv), transparent: true, depthWrite: false, opacity: 0 }));
  bg.position.set(width / 2, -h / 2 + 0.8 - LP * s * 0.5, -0.02); bg.renderOrder = 8; g.add(bg);
  const lines = rows.map((segs, i) => {
    const m = monoLine(segs); m.scale.setScalar(s); m.position.set(0.8, -i * LP * s, 0); m.renderOrder = 9; g.add(m);
    m.material.uniforms.rev.value = 999;
    return m;
  });
  // 高亮：对上的那一行底下一条工具色
  const hl = rows.map((segs, i) => {
    const m = new THREE.Mesh(new THREE.PlaneGeometry(width - 1, LP * s * 0.92), new THREE.MeshBasicMaterial({ color: 0xffffff, transparent: true, opacity: 0, depthWrite: false }));
    m.position.set(width / 2, -i * LP * s, -0.01); m.renderOrder = 8; g.add(m); return m;
  });
  return { g, bg, lines, hl, s, h };
}

// 一根线：从行尾拱起来落到面板上，按 k 画出来（平的细带子，宽 w）
function thread(p0, p1, color, C, w = 0.07) {
  const N = 64;
  const mid = p0.clone().lerp(p1, 0.5); mid.z = Math.max(p0.z, p1.z) + 3.2;
  const curve = new THREE.QuadraticBezierCurve3(p0, mid, p1);
  const pos = new Float32Array((N + 1) * 2 * 3), u = new Float32Array((N + 1) * 2);
  for (let i = 0; i <= N; i++) {
    const p = curve.getPoint(i / N), tg = curve.getTangent(i / N);
    const side = new THREE.Vector3(-tg.y, tg.x, 0).normalize().multiplyScalar(w / 2);
    pos.set([p.x + side.x, p.y + side.y, p.z, p.x - side.x, p.y - side.y, p.z], i * 6);
    u.set([i / N, i / N], i * 2);
  }
  const idx = []; for (let i = 0; i < N; i++) idx.push(i * 2, i * 2 + 1, i * 2 + 2, i * 2 + 1, i * 2 + 3, i * 2 + 2);
  const geo = new THREE.BufferGeometry(); geo.setAttribute('position', new THREE.BufferAttribute(pos, 3)); geo.setAttribute('u', new THREE.BufferAttribute(u, 1)); geo.setIndex(idx);
  const m = new THREE.Mesh(geo, new THREE.ShaderMaterial({
    uniforms: { k: { value: 0 }, alpha: { value: 1 }, color: { value: C(color).multiplyScalar(1.6) } },
    vertexShader: `attribute float u; varying float vU; void main(){ vU = u; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.); }`,
    fragmentShader: `uniform float k, alpha; uniform vec3 color; varying float vU; void main(){ float a = step(vU, k) * alpha * (.55 + .45 * smoothstep(k - .08, k, vU)); if (a < .003) discard; gl_FragColor = vec4(color, a); }`,
    transparent: true, depthWrite: false, side: THREE.DoubleSide,
  }));
  m.renderOrder = 10;
  return m;
}

export function buildEvidence({ C, T, colors }) {
  const group = new THREE.Group();
  const STAR = tex(starCanvas(128));
  const t0 = T.ev[0] + 1.2;
  // 每一行什么时候标上：从上往下，空行跳过
  let n = 0;
  const L = HERO.map((h, row) => ({ ...h, row, len: h.text.length, tag: h.text.trim() ? t0 + (n++) * 0.36 : Infinity }));

  const code = L.map((l) => { if (!l.len) return null; const m = monoLine(codeSegs(l.text)); m.position.set(0, -l.row * LP, 0.03); m.material.uniforms.rev.value = 999; m.renderOrder = 6; group.add(m); return m; });

  // 行尾的星号
  const markers = L.map((l) => {
    if (!l.len || l.kind === 'unattributed') return null;
    const m = new THREE.Mesh(new THREE.PlaneGeometry(1, 1), new THREE.MeshBasicMaterial({ map: STAR, color: C(colors[l.cls]).multiplyScalar(1.3), transparent: true, depthWrite: false, opacity: 0 }));
    m.position.set((l.len + 1.4) * CW, -l.row * LP, 0.05); m.renderOrder = 7; group.add(m);
    return m;
  });
  const KA = { verified: 1, declared: 0.6, inferred: 0.28 };
  // 未确认：一圈灰色虚线框
  const dashes = L.map((l) => {
    if (l.kind !== 'unattributed') return null;
    const w = (l.len + 1.2) * CW, h = LP * 0.92;
    const cv = document.createElement('canvas'); cv.width = Math.round(w * 60); cv.height = Math.round(h * 60);
    const g = cv.getContext('2d'); g.strokeStyle = '#8A8478'; g.lineWidth = 3; g.setLineDash([10, 8]); roundRect(g, 3, 3, cv.width - 6, cv.height - 6, 14); g.stroke();
    const m = new THREE.Mesh(new THREE.PlaneGeometry(w, h), new THREE.MeshBasicMaterial({ map: tex(cv), transparent: true, depthWrite: false, opacity: 0 }));
    m.position.set(w / 2 - CW * 0.6, -l.row * LP, 0.04); m.renderOrder = 7; group.add(m);
    return m;
  });

  // 面板 A：Claude Code 的会话记录，里面就是那几行代码
  const verified = L.filter((l) => l.kind === 'verified');
  const A = panel({ C, rows: [
    [{ text: '~/.claude/projects/-Users-dev-catnap/7f3a9c1e.jsonl', color: DIM }],
    [{ text: '"name": "Write", "file_path": "src/spots/pick.ts",', color: DIM }],
    [{ text: '"content":', color: DIM }],
    ...verified.map((l) => [{ text: '  ' + l.text, color: INK }]),
  ] });
  A.g.position.set(28, 2.6, 7.5); group.add(A.g);
  // 面板 B、C：提交信息里的 trailer
  const commit = (sha, harness, model, lines) => panel({ C, rows: [
    [{ text: `commit ${sha}`, color: '#C9B98F' }],
    [],
    [{ text: `Creditia-Harness: ${harness}`, color: INK }],
    [{ text: `Creditia-Model: ${model}`, color: INK }],
    [{ text: `Creditia-Lines: ${lines}`, color: DIM }],
  ] });
  const Bp = commit('8c1e2f4', 'codex', 'gpt-5.6-sol', 'codex 3 / 3'); Bp.g.position.set(30, -5.4, 4.6); group.add(Bp.g);
  const Cp = commit('51d0a9e', 'cursor', 'composer-2', 'cursor 2 / 2'); Cp.g.position.set(27, -12.8, 4.2); group.add(Cp.g);
  const panels = [{ p: A, first: verified[0].tag }, { p: Bp, first: L[3].tag }, { p: Cp, first: L[11].tag }];

  // 线：verified → A 里对应的那一行；declared → B / C 的 Harness 那一行
  const threads = [];
  L.forEach((l) => {
    if (l.kind !== 'verified' && l.kind !== 'declared') return;
    const p0 = new THREE.Vector3((l.len + 1.4) * CW + 0.5, -l.row * LP, 0.06);
    let P, line;
    if (l.kind === 'verified') { P = A; line = 3 + verified.indexOf(l); }
    else { P = l.cls === 1 ? Bp : Cp; line = 2; }
    const p1 = new THREE.Vector3(P.g.position.x + 0.55, P.g.position.y - line * LP * P.s, P.g.position.z + 0.02);
    const m = thread(p0, p1, colors[l.cls], C); group.add(m);
    threads.push({ m, l, P, line });
  });

  function update(t, { alpha = 1, textA = 1 } = {}) {
    L.forEach((l, i) => {
      if (code[i]) code[i].material.uniforms.alpha.value = textA * alpha;
      const k = clamp((t - l.tag) / 0.35);
      if (markers[i]) {
        const spring = k <= 0 ? 0 : 1 - Math.exp(-k * 6) * Math.cos(k * 9);
        markers[i].material.opacity = (k > 0 ? 1 : 0) * KA[l.kind] * alpha;
        markers[i].scale.setScalar(0.66 * spring + 0.0001);
      }
      if (dashes[i]) dashes[i].material.opacity = smr(t, l.tag, l.tag + 0.4) * 0.85 * alpha;
    });
    for (const { p, first } of panels) {
      const k = esin(prog(t, first - 0.3, first + 0.6));
      p.bg.material.opacity = k * alpha; p.lines.forEach((m) => (m.material.uniforms.alpha.value = k * alpha * 0.95));
      p.g.position.z = p.g.userData.z ?? (p.g.userData.z = p.g.position.z);
      p.g.position.z = p.g.userData.z * (0.35 + 0.65 * k);
    }
    for (const th of threads) {
      const k = eo3(prog(t, th.l.tag + 0.05, th.l.tag + 0.75));
      th.m.material.uniforms.k.value = k; th.m.material.uniforms.alpha.value = alpha * 0.85;
      const arrive = smr(t, th.l.tag + 0.6, th.l.tag + 0.9);
      const hl = th.P.hl[th.line];
      hl.material.color.copy(C(colors[th.l.cls])); hl.material.opacity = Math.max(hl.material.opacity * 0, arrive * 0.16 * alpha);
    }
    // 同一面板同一行（B、C 的 Harness 行）被几根线共用：取最亮的
    for (const P of [A, Bp, Cp]) P.hl.forEach((m, i) => { const ths = threads.filter((x) => x.P === P && x.line === i); if (ths.length) m.material.opacity = Math.max(...ths.map((x) => smr(t, x.l.tag + 0.6, x.l.tag + 0.9))) * 0.16 * alpha; });
  }
  // 给算账那一段：每一行行尾的位置、颜色
  const sources = L.filter((l) => l.len).map((l) => ({ x: (l.len + 1.4) * CW, y: -l.row * LP, cls: l.cls }));
  return { group, update, sources, lastTag: Math.max(...L.filter((l) => l.len).map((l) => l.tag)) };
}
