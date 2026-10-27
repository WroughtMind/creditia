// 字：一行等宽字（终端、代码）画成一张贴图，按字数从左往右「写」出来；普通的字（名字、数字）画成一块
import * as THREE from 'three';

export const LP = 1, CW = 0.387;            // 一行的高、一格字的宽（世界单位）
export const FPX = 77, HPX = 120, GX = FPX * 0.6; // 贴图里：字号、行高、一格的像素

export const tex = (cv) => {
  const t = new THREE.CanvasTexture(cv);
  t.colorSpace = THREE.SRGBColorSpace; t.anisotropy = 8; t.minFilter = THREE.LinearMipmapLinearFilter;
  return t;
};

// segs: [{ text, color, alpha }]，逐格画（等宽，不受字距影响）
export function monoCanvas(segs, { weight = 400 } = {}) {
  const n = Math.max(1, segs.reduce((s, x) => s + [...x.text].length, 0));
  const cv = document.createElement('canvas');
  cv.width = Math.ceil(n * GX + 8); cv.height = HPX;
  const g = cv.getContext('2d');
  g.textBaseline = 'middle';
  let x = 4;
  for (const s of segs) {
    g.font = `${s.bold ? 500 : weight} ${FPX}px Mono`;
    g.fillStyle = s.color; g.globalAlpha = s.alpha ?? 1;
    for (const ch of s.text) {
      // 方块字符自己画：字体里的 █ ░ 上下留白不一，拼起来会断
      if (ch === '█') g.fillRect(x, HPX * 0.16, GX + 0.5, HPX * 0.68);
      else if (ch === '░') { g.globalAlpha = (s.alpha ?? 1) * 0.2; g.fillRect(x, HPX * 0.16, GX + 0.5, HPX * 0.68); g.globalAlpha = s.alpha ?? 1; }
      else g.fillText(ch, x, HPX * 0.52);
      x += GX;
    }
  }
  return { cv, n };
}

const LINE_VS = `varying vec2 vUv; void main(){ vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.); }`;
const LINE_FS = `uniform sampler2D map; uniform float rev, n, alpha; varying vec2 vUv;
  void main(){ vec4 c = texture2D(map, vUv); float k = clamp(rev - vUv.x * n, 0., 1.); gl_FragColor = vec4(c.rgb, c.a * k * alpha); if (gl_FragColor.a < .003) discard; }`;

// 一行：左端对齐在 (0, 0)，宽 n 格；rev = 已写出的格数
export function monoLine(segs, opts) {
  const { cv, n } = monoCanvas(segs, opts);
  const w = (cv.width / GX) * CW;
  const geo = new THREE.PlaneGeometry(w, LP); geo.translate(w / 2, 0, 0);
  const m = new THREE.Mesh(geo, new THREE.ShaderMaterial({
    uniforms: { map: { value: tex(cv) }, rev: { value: 0 }, n: { value: cv.width / GX }, alpha: { value: 1 } },
    vertexShader: LINE_VS, fragmentShader: LINE_FS, transparent: true, depthWrite: false,
  }));
  m.userData.len = n;
  return m;
}

// 代码高亮：关键字暗一点，注释更暗
const KEYWORDS = /\b(export|async|function|const|let|await|for|of|if|return|fn|pub|use|impl)\b/g;
export function codeSegs(text, ink = '#E9E2D3') {
  if (text.trimStart().startsWith('//')) return [{ text, color: '#6F695F' }];
  const out = []; let last = 0;
  for (const m of text.matchAll(KEYWORDS)) { out.push({ text: text.slice(last, m.index), color: ink }); out.push({ text: m[0], color: '#958D7F' }); last = m.index + m[0].length; }
  out.push({ text: text.slice(last), color: ink });
  return out.filter((s) => s.text);
}

// 五瓣星号（同 packages/render/src/mark.ts 的 petal()）：canvas 坐标，y 朝下
export function petalPts(cx, cy, a, len, rb, gap, seg = 24) {
  const ux = Math.cos(a), uy = Math.sin(a), px = -uy, py = ux;
  const ox = cx + ux * gap, oy = cy + uy * gap, d = len - gap - rb;
  const bx = ox + ux * d, by = oy + uy * d;
  const th = Math.PI / 2 + Math.asin(Math.min(0.99, rb / d)), pts = [[ox, oy]];
  for (let k = 0; k <= seg; k++) { const t = th - (2 * th * k) / seg; pts.push([bx + rb * (Math.cos(t) * ux + Math.sin(t) * px), by + rb * (Math.cos(t) * uy + Math.sin(t) * py)]); }
  return pts;
}
export function starCanvas(px, gapK = 0) {
  const cv = document.createElement('canvas'); cv.width = cv.height = px;
  const g = cv.getContext('2d'); g.fillStyle = '#fff';
  const R = px * 0.46, c = px / 2;
  for (let i = 0; i < 5; i++) { const p = petalPts(c, c, -Math.PI / 2 + (i * 2 * Math.PI) / 5, R, R * 0.224, R * gapK); g.beginPath(); p.forEach(([x, y], k) => (k ? g.lineTo(x, y) : g.moveTo(x, y))); g.closePath(); g.fill(); }
  return cv;
}

// 一块普通的字：runs = [{ text, color, weight, family }]
export function textCanvas(runs, px, { weight = 400, family = 'Sans', pad = 0.25, spacing = 0 } = {}) {
  const cv = document.createElement('canvas'), g = cv.getContext('2d');
  const font = (r) => `${r.weight ?? weight} ${px}px ${r.family ?? family}`;
  const measure = (r) => { g.font = font(r); return g.measureText(r.text).width + spacing * px * [...r.text].length; };
  const w = runs.reduce((s, r) => s + measure(r), 0);
  cv.width = Math.ceil(w + px * pad * 2); cv.height = Math.ceil(px * 1.5);
  let x = px * pad;
  for (const r of runs) {
    g.font = font(r); g.fillStyle = r.color; g.textBaseline = 'middle';
    if (spacing) { for (const ch of r.text) { g.fillText(ch, x, px * 0.78); x += g.measureText(ch).width + spacing * px; } }
    else { g.fillText(r.text, x, px * 0.78); x += g.measureText(r.text).width; }
  }
  return cv;
}
// anchor：0 左对齐，0.5 居中，1 右对齐
export function textMesh(cv, h, anchor = 0) {
  const w = (h * cv.width) / cv.height;
  const g = new THREE.PlaneGeometry(w, h); g.translate(w * (0.5 - anchor), 0, 0);
  const m = new THREE.Mesh(g, new THREE.MeshBasicMaterial({ map: tex(cv), transparent: true, depthWrite: false, opacity: 0 }));
  m.renderOrder = 20;
  return m;
}
