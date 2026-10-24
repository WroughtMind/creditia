// creditia 短片《脚注》：每一行代码都有一个脚注。render(t) 只依赖 t，同一个 t 永远画出同一帧。
//
//   一行代码写出来，行尾冒出一个 *，下面是它的脚注（哪个 AI、哪个模型）
//   → 镜头往后拉：每一行都被各自的 AI 写出来，各冒一个 *，整个仓库亮成一片
//   → 所有的 * 离开纸面，按颜色长成这个项目自己的星号，瓣长就是份额
//   → 星号的五瓣展开成一手牌：26 种风格的同一张署名卡，一道波从左到右把每张抬起来
//   → 牌合拢，收回成星号，creditia
import * as THREE from 'three';
import { Post } from './post.js';
import { track, lerp, clamp, prog, sm, smr, esin, eio3, eo3, wob, lin, rng } from './lib.js';
import { AGENTS, UNATTRIBUTED, UN, FILE, KEYWORDS, PRINTS } from './data.js';

const params = new URLSearchParams(location.search);
const SCALE = Number(params.get('scale') ?? 1);
const PORTRAIT = params.get('aspect') === '9:16';
const W = Math.round((PORTRAIT ? 1080 : 1920) * SCALE), H = Math.round((PORTRAIT ? 1920 : 1080) * SCALE);
// 竖版：横向能看到的少了，镜头按比例退远一些（不完全退，画面不至于空）
const FIT = PORTRAIT ? Math.pow((16 / 9) / (9 / 16), 0.62) : 1;

// ───────── 时间表（秒）─────────
const T = {
  open: [0, 0.7],
  hero: [0.7, 2.05],        // 第一行写出来
  heroStar: 2.12,           // 行尾的 *
  foot: [2.35, 2.9, 3.55, 3.95], // 脚注：进、停、出
  file: 3.7,                // 同一个文件里其余的行同时开写
  pull: [3.55, 8.0],        // 镜头往后拉
  wave: [4.15, 7.5],         // 整个仓库一圈圈写出来
  gather: [7.9, 10.7],      // 所有的 * 飞向中心
  codeOut: [8.0, 9.8],      // 代码退进黑里
  petals: [10.0, 10.9],     // 星号长实
  labels: [10.5, 11.1, 12.0, 12.5],
  rise: [11.9, 12.7],       // 星号缩小，它上方浮出一叠牌
  fan: [12.45, 13.95],      // 一叠牌像一手牌那样捻开
  ripple: [13.75, 16.15],    // 一道波从左到右抬起每张牌
  close: [16.25, 17.55],    // 合拢成一叠
  deck: [17.45, 18.45],     // 一叠收回星号
  end: [18.5, 19.5],        // 星号让到左边，字标出来
  word: [18.95, 19.95],
  cmd: [19.65, 20.4],
  out: [21.3, 22.3],
  duration: 22.3,
};

const renderer = new THREE.WebGLRenderer({ antialias: false, preserveDrawingBuffer: true, powerPreference: 'high-performance' });
renderer.setPixelRatio(1); renderer.setSize(W, H);
renderer.outputColorSpace = THREE.LinearSRGBColorSpace;
document.body.appendChild(renderer.domElement);
const post = new Post(renderer, W, H);
const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(30, W / H, 0.5, 8000);

const C = (hex) => new THREE.Color().setRGB(...lin(hex));
const INK = '#E9E2D3';
const AC = [...AGENTS.map((a) => a.color), UNATTRIBUTED.color];

// 底：很暗的暖黑，像夜里的纸
const bg = new THREE.Mesh(new THREE.PlaneGeometry(40000, 40000), new THREE.MeshBasicMaterial({ color: C('#0B0A09'), depthWrite: false }));
bg.position.z = -600; bg.renderOrder = -10;
scene.add(bg);

// ───────── 排版：代码以「行距 = 1」为单位，等宽字一格 CW ─────────
const LP = 1, CW = 0.387, FPX = 77, HPX = 120; // 贴图里：行高 120px，字号 77px，一格 46.44px
const GX = FPX * 0.6;
function lineCanvas(text) {
  const n = Math.max(1, text.length), cv = document.createElement('canvas');
  cv.width = Math.ceil(n * GX + 8); cv.height = HPX;
  const g = cv.getContext('2d');
  g.font = `400 ${FPX}px Mono`; g.textBaseline = 'middle';
  const parts = [];
  if (text.trimStart().startsWith('//')) parts.push([text, '#6F695F']);
  else { let last = 0; for (const m of text.matchAll(KEYWORDS)) { parts.push([text.slice(last, m.index), INK]); parts.push([m[0], '#958D7F']); last = m.index + m[0].length; } parts.push([text.slice(last), INK]); }
  let x = 4;
  for (const [s, c] of parts) { g.fillStyle = c; for (const ch of s) { g.fillText(ch, x, HPX * 0.52); x += GX; } }
  return cv;
}
const tex = (cv) => { const t = new THREE.CanvasTexture(cv); t.colorSpace = THREE.SRGBColorSpace; t.anisotropy = 8; t.generateMipmaps = true; t.minFilter = THREE.LinearMipmapLinearFilter; return t; };

// 一行代码：一块贴图，从左往右「写」出来（rev = 已经写出的字数）
const LINE_VS = `varying vec2 vUv; void main(){ vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.); }`;
const LINE_FS = `uniform sampler2D map; uniform float rev, n, alpha; varying vec2 vUv;
  void main(){ vec4 c = texture2D(map, vUv); float x = vUv.x * n; float k = clamp(rev - x, 0., 1.); gl_FragColor = vec4(c.rgb, c.a * k * alpha); if (gl_FragColor.a < .003) discard; }`;
const fileLines = FILE.map(([text, agent], row) => {
  const cv = lineCanvas(text), n = cv.width / GX;
  const m = new THREE.Mesh(new THREE.PlaneGeometry(n * CW, LP), new THREE.ShaderMaterial({
    uniforms: { map: { value: tex(cv) }, rev: { value: 0 }, n: { value: n }, alpha: { value: 1 } },
    vertexShader: LINE_VS, fragmentShader: LINE_FS, transparent: true, depthWrite: false,
  }));
  m.position.set((n * CW) / 2, -row * LP, 0);
  scene.add(m);
  return { m, text, agent, row, len: text.length };
});
// 每一行什么时候写：第一行单独写；其余的在镜头开始往后拉时一起写，各自一个光标、各自的速度
const fr = rng(7);
fileLines.forEach((L, i) => {
  if (i === 0) { L.t0 = T.hero[0]; L.t1 = T.hero[1]; return; }
  L.t0 = T.file + fr() * 0.35 + (i % 3) * 0.08;
  L.t1 = L.t0 + 0.45 + L.len * 0.018 + fr() * 0.25;
});
// 写的节奏：一小块一小块地冒（模型是按 token 吐字的），不是匀速打字
function revAt(L, t) {
  const k = prog(t, L.t0, L.t1);
  if (k <= 0) return 0; if (k >= 1) return L.len + 1;
  const toks = Math.ceil(L.len / 3.2), j = Math.floor(eo3(k) * toks * 0.999 + 0.0001);
  return Math.min(L.len, Math.round(((j + 1) * L.len) / toks));
}
// 光标：工具色的一小竖条，写之前在原地闪
const carets = fileLines.map((L) => {
  const m = new THREE.Mesh(new THREE.PlaneGeometry(CW * 0.16, LP * 0.78), new THREE.MeshBasicMaterial({ color: C(AC[Math.min(L.agent, UN)]).multiplyScalar(1.6), transparent: true, depthWrite: false }));
  m.position.z = 0.01; scene.add(m); return m;
});

// ───────── 星号（脚注的 *）：单色的五瓣，贴图画一次，每颗按工具上色 ─────────
function petalPts(cx, cy, a, len, rb, gap, seg = 24) {
  const ux = Math.cos(a), uy = Math.sin(a), px = -uy, py = ux;
  const ox = cx + ux * gap, oy = cy + uy * gap, d = len - gap - rb;
  const bx = ox + ux * d, by = oy + uy * d;
  const th = Math.PI / 2 + Math.asin(Math.min(0.99, rb / d)), pts = [[ox, oy]];
  for (let k = 0; k <= seg; k++) { const t = th - (2 * th * k) / seg; pts.push([bx + rb * (Math.cos(t) * ux + Math.sin(t) * px), by + rb * (Math.cos(t) * uy + Math.sin(t) * py)]); }
  return pts;
}
// 和 packages/render/src/mark.ts 同一个形：瓣从正上方顺时针（屏幕上 y 朝下，这里 y 朝上，所以角度取负）
const petalAngle = (i, n = 5) => Math.PI / 2 - (i * 2 * Math.PI) / n;
function starCanvas(px) {
  const cv = document.createElement('canvas'); cv.width = cv.height = px;
  const g = cv.getContext('2d'); g.fillStyle = '#fff';
  const R = px * 0.46, c = px / 2;
  for (let i = 0; i < 5; i++) { const a = -Math.PI / 2 + (i * 2 * Math.PI) / 5; const p = petalPts(c, c, a, R, R * 0.224, 0); g.beginPath(); p.forEach(([x, y], k) => (k ? g.lineTo(x, y) : g.moveTo(x, y))); g.closePath(); g.fill(); }
  return cv;
}
const STAR = tex(starCanvas(128));

// ───────── 整个仓库：围着 pick.ts 摊开的一片代码，一个文件一段 ─────────
const COLW = 46, COLP = COLW * CW + 7; // 一列宽 46 格，列距
const MAP = { cols: 27, rows: 380 };
const mr = rng(31);
const strokes = []; // [x, y, w, tb, shade]
const stars = [];   // { x, y, agent, tPop, size }
const heroC = { x: 9.6, y: -7.5 }; // pick.ts 的中心，镜头拉远、星号生长都围着它
const pickAgent = () => { let v = mr() * (100 - UNATTRIBUTED.pct * 0.4); for (let k = 0; k < AGENTS.length; k++) { v -= AGENTS[k].pct; if (v <= 0) return k; } return UN; };
const c0 = Math.floor(MAP.cols / 2), r0 = Math.floor(MAP.rows * 0.47);
const maxD = Math.hypot(c0 * COLP, MAP.rows * 0.55);
for (let c = 0; c < MAP.cols; c++) {
  let r = Math.floor(mr() * 6);
  while (r < MAP.rows) {
    let n = Math.max(8, Math.round(Math.exp(2.6 + mr() * 1.7)));
    const x0 = (c - c0) * COLP;
    // pick.ts 自己占着中间那一列的这一段
    if (c === c0 && r < r0 + 18 && r + n > r0 - 2) { if (r0 - 2 - r >= 8) n = r0 - 2 - r; else { r = r0 + 18; continue; } }
    const dom = pickAgent(), fileT = lerp(T.wave[0], T.wave[1], Math.pow(clamp(Math.hypot(x0, (r - r0) * LP) / maxD), 0.75)) + mr() * 0.5;
    let agent = dom, block = 0, ind = 0, hunk = 0;
    for (let i = 0; i < n; i++) {
      if (block-- <= 0) { block = 5 + Math.floor(mr() * 30); agent = mr() < 0.78 ? dom : pickAgent(); }
      if (i > 0 && mr() < 0.12) continue; // 空行
      ind = i === 0 ? 0 : Math.max(0, Math.min(4, ind + (mr() < 0.2 ? 1 : mr() < 0.25 ? -1 : 0)));
      const y = -(r + i - r0) * LP + heroC.y + 7.5, tb = fileT + i * 0.012;
      let x = x0 + ind * 2 * CW; const end = x + Math.max(4, (COLW - ind * 2) * (0.2 + mr() * 0.75)) * CW;
      let first = true;
      while (x < end - CW) { const w = Math.min(end - x, (2 + Math.floor(mr() * 9)) * CW); strokes.push([x, y, w, tb + (x - x0) * 0.004, first ? 0.55 : mr() < 0.5 ? 0.85 : 0.68]); x += w + CW; first = false; }
      if (agent !== UN && (hunk-- <= 0 || block === 0)) { hunk = 1 + Math.floor(mr() * 4); stars.push({ x: end + CW * 1.2, y: y + LP * 0.3, agent, tPop: tb + 0.22 + (end - x0) * 0.004, size: 0.62 }); }
    }
    r += n + 2;
  }
}
// pick.ts 自己那几行的星号：行写完时冒出来
fileLines.forEach((L) => { if (L.agent !== UN && L.len) stars.push({ x: L.len * CW + CW * 1.1, y: -L.row * LP + LP * 0.3, agent: L.agent, tPop: L.row === 0 ? T.heroStar : L.t1 + 0.04, size: 0.62, file: true }); });

const quad = new THREE.PlaneGeometry(1, 1);
const strokeGeo = new THREE.InstancedBufferGeometry();
strokeGeo.index = quad.index; strokeGeo.setAttribute('position', quad.getAttribute('position'));
strokeGeo.setAttribute('a0', new THREE.InstancedBufferAttribute(new Float32Array(strokes.flatMap((s) => [s[0], s[1], s[2], s[3]])), 4));
strokeGeo.setAttribute('a1', new THREE.InstancedBufferAttribute(new Float32Array(strokes.map((s) => s[4])), 1));
strokeGeo.instanceCount = strokes.length;
// 一笔 = 一个词：从左往右长出来（a0 = x, y, 宽, 开写的时刻；a1 = 深浅）
const strokeMat = new THREE.ShaderMaterial({
  uniforms: { t: { value: 0 }, alpha: { value: 1 }, ink: { value: C(INK) } },
  vertexShader: `attribute vec4 a0; attribute float a1; uniform float t; varying float vA;
    void main(){
      float k = clamp((t - a0.w) / .18, 0., 1.);
      vec3 p = vec3(a0.x + (position.x + .5) * a0.z * k, a0.y + position.y * .44, 0.);
      vA = a1 * step(.001, k);
      gl_Position = projectionMatrix * modelViewMatrix * vec4(p, 1.);
    }`,
  fragmentShader: `uniform float alpha; uniform vec3 ink; varying float vA; void main(){ gl_FragColor = vec4(ink, vA * alpha); if (gl_FragColor.a < .003) discard; }`,
  transparent: true, depthWrite: false,
});
const strokeMesh = new THREE.Mesh(strokeGeo, strokeMat); strokeMesh.frustumCulled = false;
scene.add(strokeMesh);

// ───────── 大星号：这个项目自己的 *，瓣长按份额（同 mark.ts 的 sharePetals） ─────────
const MARK = { x: heroC.x, y: heroC.y, R: 62 };
const maxPct = AGENTS[0].pct;
const shareLen = AGENTS.map((a) => 0.46 + 0.54 * Math.sqrt(a.pct / maxPct));
function petalShape(i, len, R, gap = R * 0.07) {
  const pts = petalPts(0, 0, petalAngle(i), R * len, R * 0.224 * Math.sqrt(len), gap);
  return new THREE.Shape(pts.map(([x, y]) => new THREE.Vector2(x, y)));
}
const petalMeshes = AGENTS.map((a, i) => {
  const m = new THREE.Mesh(new THREE.ShapeGeometry(petalShape(i, shareLen[i], 1), 4), new THREE.MeshBasicMaterial({ color: C(a.color), transparent: true, depthWrite: false }));
  m.renderOrder = 100; scene.add(m); return m; // 画在牌上面
});
let petalLenNow = null;
function setPetals(lens) {
  const key = lens.map((v) => v.toFixed(4)).join();
  if (key === petalLenNow) return; petalLenNow = key;
  petalMeshes.forEach((m, i) => { m.geometry.dispose(); m.geometry = new THREE.ShapeGeometry(petalShape(i, lens[i], 1), 4); });
}
// 星星的落点：在自己那一瓣里均匀撒（拒绝采样），瓣越长、落的越多
function inPetal(px, py, i, len) {
  const a = petalAngle(i), ux = Math.cos(a), uy = Math.sin(a), gap = 0.07, rb = 0.224 * Math.sqrt(len);
  const along = px * ux + py * uy, across = Math.abs(-px * uy + py * ux), d = len - gap - rb;
  if (along < gap || along > len) return false;
  const bx = ux * (gap + d), by = uy * (gap + d);
  if (Math.hypot(px - bx, py - by) <= rb) return true;
  return along <= gap + d && across <= rb * ((along - gap) / d) * 1.02;
}
const sr = rng(5);
for (const s of stars) {
  const i = s.agent, len = shareLen[i];
  let x, y; do { x = (sr() * 2 - 1) * len; y = (sr() * 2 - 1) * len; } while (!inPetal(x, y, i, len));
  s.tx = MARK.x + x * MARK.R; s.ty = MARK.y + y * MARK.R;
  s.delay = clamp(Math.hypot(s.x - MARK.x, s.y - MARK.y) / 260) * 1.1 + sr() * 0.45;
  s.swirl = 0.7 + sr() * 0.6;
}
const starGeo = new THREE.InstancedBufferGeometry();
starGeo.index = quad.index; starGeo.setAttribute('position', quad.getAttribute('position')); starGeo.setAttribute('uv', quad.getAttribute('uv'));
starGeo.setAttribute('s0', new THREE.InstancedBufferAttribute(new Float32Array(stars.flatMap((s) => [s.x, s.y, s.tPop, s.size])), 4));
starGeo.setAttribute('s1', new THREE.InstancedBufferAttribute(new Float32Array(stars.flatMap((s) => [s.tx, s.ty, s.delay, s.swirl])), 4));
starGeo.setAttribute('col', new THREE.InstancedBufferAttribute(new Float32Array(stars.flatMap((s) => { const c = C(AC[s.agent]); return [c.r, c.g, c.b]; })), 3));
starGeo.instanceCount = stars.length;
const starMat = new THREE.ShaderMaterial({
  uniforms: { map: { value: STAR }, t: { value: 0 }, grow: { value: 1 }, g0: { value: T.gather[0] }, gdur: { value: 1.55 }, center: { value: new THREE.Vector2(MARK.x, MARK.y) }, tsize: { value: 2.2 }, fade: { value: 1 }, glow: { value: 1.5 } },
  vertexShader: `attribute vec4 s0, s1; attribute vec3 col; uniform float t, grow, g0, gdur, tsize; uniform vec2 center; varying vec2 vUv; varying vec3 vC; varying float vA;
    float spring(float x){ if (x <= 0.) return 0.; return 1. - exp(-x * 9.) * cos(x * 17.); }
    void main(){
      vUv = uv; vC = col;
      float pop = spring(t - s0.z);
      float k = clamp((t - g0 - s1.z) / gdur, 0., 1.);
      float e = k * k * (3. - 2. * k);
      vec2 a = s0.xy - center, b = s1.xy - center;
      vec2 p = mix(a, b, e);
      float r = sin(3.14159 * e) * s1.w * 1.1;                        // 飞的路上绕着中心旋一下，起点、落点都不偏
      p = vec2(cos(r) * p.x - sin(r) * p.y, sin(r) * p.x + cos(r) * p.y);
      float z = sin(3.14159 * e) * 38. * s1.w;
      float sz = mix(s0.w * grow, tsize, e) * pop * (1. + .9 * sin(3.14159 * e));
      vA = step(.0001, pop);
      vec3 pos = vec3(center + p + position.xy * sz, .02 + z);
      gl_Position = projectionMatrix * modelViewMatrix * vec4(pos, 1.);
    }`,
  fragmentShader: `uniform sampler2D map; uniform float fade, glow; varying vec2 vUv; varying vec3 vC; varying float vA;
    void main(){ float a = texture2D(map, vUv).a * vA * fade; if (a < .01) discard; gl_FragColor = vec4(vC * glow, a); }`,
  transparent: true, depthWrite: false,
});
const starMesh = new THREE.Mesh(starGeo, starMat); starMesh.frustumCulled = false; starMesh.renderOrder = 4;
scene.add(starMesh);

// ───────── 字：脚注、瓣尖的名字和份额、字标 ─────────
function textCanvas(runs, px, { weight = 400, family = 'Sans', pad = 0.25 } = {}) {
  const cv = document.createElement('canvas'), g = cv.getContext('2d');
  const font = (r) => `${r.weight ?? weight} ${px}px ${r.family ?? family}`;
  let w = 0; for (const r of runs) { g.font = font(r); w += r.star ? px * 0.78 : g.measureText(r.text).width; }
  cv.width = Math.ceil(w + px * pad * 2); cv.height = Math.ceil(px * 1.5);
  let x = px * pad;
  for (const r of runs) {
    if (r.star) { const s = px * 0.46, c = { x: x + s * 0.5, y: px * 0.55 }; g.fillStyle = r.color; for (let i = 0; i < 5; i++) { const p = petalPts(c.x, c.y, -Math.PI / 2 + (i * 2 * Math.PI) / 5, s * 0.5, s * 0.112, 0); g.beginPath(); p.forEach(([a, b], k) => (k ? g.lineTo(a, b) : g.moveTo(a, b))); g.fill(); } x += px * 0.78; continue; }
    g.font = font(r); g.fillStyle = r.color; g.textBaseline = 'middle'; g.fillText(r.text, x, px * 0.78); x += g.measureText(r.text).width;
  }
  return cv;
}
function textMesh(cv, h, anchor = 0) {
  const w = (h * cv.width) / cv.height;
  const g = new THREE.PlaneGeometry(w, h); g.translate(w * (0.5 - anchor), 0, 0);
  const m = new THREE.Mesh(g, new THREE.MeshBasicMaterial({ map: tex(cv), transparent: true, depthWrite: false, opacity: 0 }));
  m.renderOrder = 6; scene.add(m); return m;
}
const hero = AGENTS[0];
const foot = textMesh(textCanvas([{ star: true, color: hero.color }, { text: `${hero.name}`, color: INK, weight: 500 }, { text: `  ·  ${hero.model}`, color: '#A39B8B' }], 96), 0.95);
foot.position.set(-0.12, -2.35, 0);
const rule = new THREE.Mesh(new THREE.PlaneGeometry(1, 0.035), new THREE.MeshBasicMaterial({ color: C('#6F695F'), transparent: true, opacity: 0, depthWrite: false }));
rule.geometry.translate(0.5, 0, 0); rule.position.set(0.05, -1.55, 0); scene.add(rule);
const labels = AGENTS.map((a, i) => {
  const ang = petalAngle(i), left = Math.cos(ang) < -0.2;
  const m = textMesh(textCanvas([{ text: a.name, color: INK, weight: 500 }, { text: `  ${a.pct}%`, color: a.color, weight: 500 }], 96), 5.6, left ? 1 : Math.abs(Math.cos(ang)) <= 0.2 ? 0.5 : 0);
  const r = MARK.R * shareLen[i] + 7;
  m.position.set(MARK.x + Math.cos(ang) * r, MARK.y + Math.sin(ang) * r + (Math.abs(Math.cos(ang)) <= 0.2 ? 3 : 0), 0);
  return m;
});
const word = textMesh(textCanvas([{ text: 'creditia', color: INK, weight: 600 }], 200, { pad: 0.08 }), 15.5, 0);
const cmd = textMesh(textCanvas([{ text: 'npx creditia analyze', color: '#8F887B', family: 'Mono' }], 96, { pad: 0.05 }), 3.4, 0);

// ───────── 一手牌：26 种风格的同一张署名卡 ─────────
const loader = new THREE.TextureLoader();
const cards = await Promise.all(PRINTS.map(async (id, i) => {
  const t = await loader.loadAsync(`/promo/.build/short/tex/${id}.jpg`);
  t.colorSpace = THREE.SRGBColorSpace; t.anisotropy = 8;
  const asp = t.image.width / t.image.height, h = 26, w = h * asp;
  const geo = new THREE.PlaneGeometry(w, h); geo.translate(0, h / 2, 0); // 原点在牌的下沿中点：扇面绕它转
  const m = new THREE.Mesh(geo, new THREE.ShaderMaterial({
    uniforms: { map: { value: t }, alpha: { value: 0 }, lift: { value: 0 }, size: { value: new THREE.Vector2(w, h) } },
    vertexShader: `varying vec2 vUv; void main(){ vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.); }`,
    // 圆角 + 被抬起时纸面稍亮一点；下一张压着的地方有一道很淡的阴影（由牌自己的左边缘画）
    fragmentShader: `uniform sampler2D map; uniform float alpha, lift; uniform vec2 size; varying vec2 vUv;
      void main(){
        vec2 p = (vUv - .5) * size; vec2 q = abs(p) - size * .5 + 1.1; float d = length(max(q, 0.)) - 1.1;
        float a = 1. - smoothstep(-.12, .12, d);
        vec3 c = texture2D(map, vUv).rgb * (.8 + .12 * lift);
        float edge = smoothstep(0., 3.5, (vUv.x) * size.x);
        c *= mix(.72, 1., edge);
        gl_FragColor = vec4(c, a * alpha); if (gl_FragColor.a < .003) discard;
      }`,
    transparent: true, depthWrite: true,
  }));
  // 牌底下一块软阴影
  const sh = new THREE.Mesh(new THREE.PlaneGeometry(w * 1.12, h * 1.18), new THREE.ShaderMaterial({
    uniforms: { alpha: { value: 0 } },
    vertexShader: `varying vec2 vUv; void main(){ vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.); }`,
    fragmentShader: `uniform float alpha; varying vec2 vUv; void main(){ vec2 d = abs(vUv - .5) * 2.; float k = (1. - smoothstep(.62, 1., d.x)) * (1. - smoothstep(.55, 1., d.y)); gl_FragColor = vec4(0., 0., 0., k * alpha * .55); }`,
    transparent: true, depthWrite: false,
  }));
  sh.geometry.translate(0, h * 0.55, 0);
  const grp = new THREE.Group(); grp.add(sh, m); scene.add(grp);
  return { grp, m, sh, w, h, i };
}));
const NC = cards.length, FAN = 200 * Math.PI / 180, HUB = 58; // 扇面张开的角度，牌下沿离轴心的距离

// ───────── 事件表：配乐按它对齐 ─────────
window.EVENTS = {
  duration: T.duration, T,
  pops: stars.filter((s) => s.file || s.tPop < T.gather[0]).map((s) => [+s.tPop.toFixed(3), s.agent, s.file ? 1 : 0]).sort((a, b) => a[0] - b[0]),
  ripple: cards.map((c) => [+(lerp(T.ripple[0], T.ripple[1], (c.i + 0.5) / NC)).toFixed(3), c.i]),
};

// ───────── 镜头 ─────────
// [中心 x, 中心 y, log(距离), 俯仰（度）]；距离走对数，往后拉时速度是均匀「变小」的感觉
const mcx = MARK.x, mcy = MARK.y;
const cam = track([
  { t: 0, v: [12.6, -1.15, Math.log(30), 0] },
  { t: T.pull[0], v: [12.2, -1.5, Math.log(36), 1.5], ease: (k) => esin(k) },
  { t: 6.1, v: [10.5, -6.0, Math.log(210), 5] },
  { t: T.pull[1], v: [mcx, mcy, Math.log(640), 7] },
  { t: T.gather[0] + 1.6, v: [mcx, mcy, Math.log(470), 4] },
  { t: T.petals[1], v: [mcx, mcy + 2, Math.log(300), 0], hold: true },
  { t: T.labels[3], v: [mcx, mcy + 4, Math.log(312), 0] },
  { t: T.fan[1], v: [mcx, mcy + 24, Math.log(292), -6] },
  { t: T.ripple[1], v: [mcx, mcy + 25, Math.log(272), -8] },
  { t: T.deck[1], v: [mcx, mcy + 6, Math.log(165), 0], hold: true },
  { t: T.end[1], v: [mcx + 26, mcy, Math.log(160), 0] },
  { t: T.duration, v: [mcx + 26, mcy, Math.log(150), 0] },
]);

// ───────── 一帧 ─────────
function render(t) {
  const [cx, cy, ld, tilt] = cam(t);
  const d = Math.exp(ld) * FIT;
  const th = (tilt * Math.PI) / 180, sway = wob(t * 0.35, 3) * 0.6;
  camera.position.set(cx + wob(t * 0.3, 1) * d * 0.004, cy - Math.sin(th) * d, Math.cos(th) * d);
  camera.up.set(Math.sin(sway * 0.004), 1, 0);
  camera.lookAt(cx, cy, 0);

  // 代码行与光标
  const codeA = 1 - smr(t, T.codeOut[0], T.codeOut[1]);
  fileLines.forEach((L, i) => {
    const rev = revAt(L, t); L.m.material.uniforms.rev.value = rev; L.m.material.uniforms.alpha.value = codeA;
    const cr = carets[i], typing = t >= L.t0 - (i === 0 ? 0.7 : 0.15) && t < L.t1 + 0.25;
    const blink = t < L.t0 ? (Math.floor((t - 0) * 2.4) % 2 === 0 ? 1 : 0.15) : 1;
    cr.visible = typing && L.len > 0; cr.material.opacity = blink * (i === 0 ? 1 : 0.9) * (t > L.t1 ? 1 - prog(t, L.t1, L.t1 + 0.25) : 1);
    cr.position.set(Math.min(rev, L.len) * CW + CW * 0.12, -L.row * LP, 0.01);
  });
  strokeMat.uniforms.t.value = t; strokeMat.uniforms.alpha.value = 0.42 * codeA * (1 - 0.45 * smr(Math.log(d), Math.log(80), Math.log(500)));

  // 脚注
  const fa = smr(t, T.foot[0], T.foot[1]) * (1 - smr(t, T.foot[2], T.foot[3]));
  foot.material.opacity = fa; foot.position.y = -2.35 - (1 - smr(t, T.foot[0], T.foot[1])) * 0.35;
  rule.material.opacity = fa * 0.9; rule.scale.x = 0.001 + 5.2 * eo3(prog(t, T.foot[0] - 0.1, T.foot[1]));

  // 星星：远了也要看得见，所以随镜头距离长大一点；落进星号之后淡掉，换成实心的瓣
  starMat.uniforms.t.value = t;
  starMat.uniforms.grow.value = Math.pow(Math.max(1, d / 34), 0.42);
  starMat.uniforms.fade.value = 1 - smr(t, T.petals[0] + 0.25, T.petals[1]);
  starMat.uniforms.glow.value = 1.35 + 0.5 * smr(t, T.gather[0], T.gather[0] + 1.5);

  // 大星号：长实 → 展开成牌时缩到轴心 → 收牌后变回标准的五瓣（等长）
  const pk = smr(t, T.petals[0], T.petals[1]);
  const shrink = esin(prog(t, T.rise[0] - 0.3, T.fan[0] + 0.6));
  const back = esin(prog(t, T.deck[0] + 0.2, T.deck[1]));
  const lens = shareLen.map((l) => lerp(l, 1, back));
  setPetals(lens);
  const endK = esin(prog(t, T.end[0], T.end[1]));
  const R = lerp(lerp(MARK.R, 15, shrink), 9.5, endK) * (1 + 0.04 * Math.sin(Math.PI * back));
  const mx = lerp(MARK.x, MARK.x + 26 - 30.5, endK);
  const spin = -0.22 * Math.sin(Math.PI * esin(prog(t, T.deck[0], T.deck[1] + 0.5)));
  const markA = pk * (1 - smr(t, T.out[0], T.out[1]));
  petalMeshes.forEach((m) => { m.scale.setScalar(R); m.position.set(mx, MARK.y, 0.05); m.rotation.z = spin; m.material.opacity = markA; });

  // 瓣尖的名字和份额
  const la = smr(t, T.labels[0], T.labels[1]) * (1 - smr(t, T.labels[2], T.labels[3]));
  labels.forEach((m, i) => { m.material.opacity = la; });

  // 牌：一叠从星号身后升起 → 捻开成一手牌 → 一道波抬起每一张 → 合拢 → 收回星号
  const rise = esin(prog(t, T.rise[0], T.rise[1]));
  const open = esin(prog(t, T.fan[0], T.fan[1]));
  const close = esin(prog(t, T.close[0], T.close[1]));
  const gone = esin(prog(t, T.deck[0], T.deck[1]));
  const wavePos = lerp(-2.5, NC + 1.5, prog(t, T.ripple[0], T.ripple[1]));
  const al = smr(t, T.rise[0], T.rise[0] + 0.55) * (1 - smr(t, T.deck[0] + 0.05, T.deck[0] + 0.6));
  for (const c of cards) {
    const fanA = Math.PI / 2 + FAN / 2 - (FAN * (c.i + 0.5)) / NC;      // 从左到右
    const a = lerp(Math.PI / 2, lerp(Math.PI / 2, fanA, open), 1 - close);
    const w = t > T.ripple[0] - 0.4 && t < T.ripple[1] + 0.4 ? Math.exp(-((c.i - wavePos) ** 2) / 1.8) : 0;
    const r = HUB * lerp(0.62, 1, rise) * (1 - 0.6 * gone) + w * 7;
    const s = lerp(0.55, 1, rise) * lerp(1, 0.35, gone) * (1 + 0.06 * w);
    c.grp.position.set(mx + Math.cos(a) * r, MARK.y + Math.sin(a) * r, -1.8 + c.i * 0.06 + w * 6);
    c.grp.rotation.z = a - Math.PI / 2;
    c.grp.scale.setScalar(Math.max(0.0001, s));
    c.m.material.uniforms.alpha.value = al; c.m.material.uniforms.lift.value = w;
    c.sh.material.uniforms.alpha.value = al * (0.55 + 0.45 * w);
    c.sh.position.set(0.6 + w * 1.6, -0.8 - w * 2.5, -0.05 - w * 3);
    c.grp.visible = al > 0.002;
    c.m.renderOrder = 10 + c.i + (w > 0.3 ? 40 : 0);
  }

  // 字标和命令
  const wk = smr(t, T.word[0], T.word[1]);
  word.material.opacity = wk * (1 - smr(t, T.out[0], T.out[1]));
  word.position.set(mx + R * 1.45 + (1 - eo3(prog(t, T.word[0], T.word[1]))) * 3, MARK.y - 0.6, 0.1);
  cmd.material.opacity = smr(t, T.cmd[0], T.cmd[1]) * (1 - smr(t, T.out[0], T.out[1]));
  cmd.position.set(mx + R * 1.45 + 0.4, MARK.y - 13.2, 0.1);

  // 镜头：对焦在纸面（星星飞起来时虚一点）；后期
  const dof = { focus: d, aperture: lerp(0.9, 0.35, smr(Math.log(d), Math.log(40), Math.log(400))) * d * 0.0016, maxCoc: 14 * SCALE + 4 };
  const fade = smr(t, T.open[0], T.open[1]) * (1 - smr(t, T.out[0], T.out[1]) * 0.98);
  const fanK = smr(t, T.rise[0], T.fan[0]) * (1 - smr(t, T.deck[0], T.deck[1]));
  post.render(scene, null, camera, dof, { bloom: lerp(0.13, 0.05, fanK), exposure: 1.0, fade, grain: 0.03, frame: Math.round(t * 60), vignette: 0.42, ca: 0.008, lift: 0.0 });
}

window.render = render;
render(0);
window.READY = true;
window.STATS = { strokes: strokes.length, stars: stars.length };
