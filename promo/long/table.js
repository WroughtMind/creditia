// 第 5 段：看样台。灯亮起来：署名卡原来躺在一张台子上，四周摊着同一份署名的 26 种样子（prints.sh 导出的真作品）
// 几件会动的在台上自己动着：霓虹在闪、CRT 的亮带往下走、玻璃上一道反光扫过
// 坐标：地图坐标；光照按世界坐标算（灯是一盏顶灯，照出一圈）
import * as THREE from 'three';
import { rng, smr, clamp } from '../short/lib.js';
import { GLSL_NOISE } from '../short/lib.js';

export const PRINTS = [
  ['minimal-light', 'share'], ['minimal-dark', 'codemap'], ['glass', 'donut', 3], ['film', 'roll'], ['cinema', 'tier'], ['editorial', 'versions'],
  ['broadsheet', 'areas'], ['playbill', 'receipt'], ['gala', 'waffle'], ['mono', 'calendar'], ['poster', 'share'], ['terminal', 'fetch'],
  ['crt', 'lanes', 2], ['neon', 'tier', 1], ['arcade', 'clock'], ['ink', 'share'], ['ukiyoe', 'share'], ['woodblock', 'roll'],
  ['baroque', 'receipt'], ['grimoire', 'tier'], ['manuscript', 'evidence'], ['pastel', 'donut'], ['cutout', 'stream'], ['meadow', 'survival'],
  ['manga', 'share'], ['cyber', 'mosaic'],
];

const LIGHT = /* glsl */ `
  uniform vec2 lc; uniform float lr, li;
  float light(vec3 w){ float d = length(w.xz - lc); return li * (.06 + .94 * exp(-pow(d / lr, 2.))); }`;

export async function buildTable({ C, card, Lw, lo = false }) {
  const P = lo ? 'prints-lo' : 'prints';
  const group = new THREE.Group();
  const r = rng(9);
  const loader = new THREE.TextureLoader();
  const center = { x: card.x + card.w / 2, y: card.y - card.h / 2 };
  const lc = new THREE.Vector2(...(() => { const w = Lw(center.x, center.y); return [w[0], w[2]]; })());
  const U = { lc: { value: lc }, lr: { value: 30 }, li: { value: 0 }, t: { value: 0 } };

  // 台面：暖色的暗木纹/纸面，只在灯下看得见
  const top = new THREE.Mesh(new THREE.PlaneGeometry(1800, 1300), new THREE.ShaderMaterial({
    uniforms: { ...U, alpha: { value: 0 } },
    vertexShader: `varying vec3 vW; varying vec2 vP; void main(){ vec4 w = modelMatrix * vec4(position, 1.); vW = w.xyz; vP = position.xy; gl_Position = projectionMatrix * viewMatrix * w; }`,
    fragmentShader: `${GLSL_NOISE} ${LIGHT} uniform float alpha; varying vec3 vW; varying vec2 vP;
      void main(){
        float grain = fbm(vP * vec2(.05, .9)) * .5 + fbm(vP * .35) * .5;
        vec3 base = vec3(.036, .025, .017) * (.78 + .44 * grain);   // 深色胡桃木
        gl_FragColor = vec4(base * light(vW) * 2.4, alpha);
      }`,
    transparent: true, depthWrite: false,
  }));
  top.position.set(center.x, center.y, -0.08); top.renderOrder = 1; group.add(top);

  const VS = `varying vec2 vUv; varying vec3 vW; void main(){ vUv = uv; vec4 w = modelMatrix * vec4(position, 1.); vW = w.xyz; gl_Position = projectionMatrix * viewMatrix * w; }`;
  const FS = `${LIGHT} ${GLSL_NOISE} uniform sampler2D map; uniform float alpha, self, mode, t, lift; uniform vec2 size; varying vec2 vUv; varying vec3 vW;
    void main(){
      vec2 p = (vUv - .5) * size; vec2 q = abs(p) - size * .5 + .9; float d = length(max(q, 0.)) - .9;
      float a = 1. - smoothstep(-.1, .1, d);
      vec3 c = texture2D(map, vUv).rgb;
      if (mode > .5 && mode < 1.5) { float f = hash12(vec2(floor(t * 14.), 3.)); c *= .84 + .16 * step(.12, f) + .1 * sin(t * 31.); }               // 霓虹在闪
      if (mode > 1.5 && mode < 2.5) { float y = fract(vUv.y + t * .23); c += c * .35 * exp(-pow((y - .5) * 14., 2.)); }                          // CRT 的亮带
      if (mode > 2.5) { float s = vUv.x * .8 + vUv.y * .5 - fract(t * .18) * 2.6 + .6; c += vec3(.22) * exp(-s * s * 60.); }                      // 玻璃上的反光
      float L = mix(light(vW) * .95, 1., self);
      gl_FragColor = vec4(c * L * (.92 + .08 * lift), a * alpha);
      if (gl_FragColor.a < .003) discard;
    }`;
  const SH_FS = `${LIGHT} uniform float alpha; varying vec2 vUv; varying vec3 vW; void main(){ vec2 d = abs(vUv - .5) * 2.; float k = (1. - smoothstep(.7, 1., d.x)) * (1. - smoothstep(.7, 1., d.y)); gl_FragColor = vec4(0., 0., 0., k * alpha * .6 * clamp(light(vW) * 3., 0., 1.)); }`;

  async function print(url, w, h, mode = 0) {
    const t = await loader.loadAsync(url); t.colorSpace = THREE.SRGBColorSpace; t.anisotropy = 8;
    const m = new THREE.Mesh(new THREE.PlaneGeometry(w, h), new THREE.ShaderMaterial({
      uniforms: { ...U, map: { value: t }, alpha: { value: 0 }, self: { value: 0 }, mode: { value: mode }, lift: { value: 0 }, size: { value: new THREE.Vector2(w, h) } },
      vertexShader: VS, fragmentShader: FS, transparent: true, depthWrite: false,
    }));
    const sh = new THREE.Mesh(new THREE.PlaneGeometry(w * 1.1, h * 1.14), new THREE.ShaderMaterial({ uniforms: { ...U, alpha: { value: 0 } }, vertexShader: VS, fragmentShader: SH_FS, transparent: true, depthWrite: false }));
    sh.position.set(0.9, -1.3, -0.04);
    const g = new THREE.Group(); g.add(sh, m); group.add(g);
    m.renderOrder = 12; sh.renderOrder = 11;
    return { g, m, sh, w, h };
  }

  // 主角：算账那一段浮出来的署名卡
  const hero = await print(`/promo/.build/long/tex/${lo ? 'card-lo' : 'card'}.jpg`, card.w, card.h);
  hero.g.position.set(center.x, center.y, 0.15); hero.m.renderOrder = 14;

  // 其余 26 件：在主角四周摊开，一格一件，角度、位置都随手一点
  const cells = [];
  for (let j = -3; j <= 3; j++) for (let i = -4; i <= 4; i++) if (i || j) cells.push([i, j, Math.hypot(i * 1.0, j * 1.35)]);
  cells.sort((a, b) => a[2] - b[2]);
  const CX = 84, CY = 64;
  const others = [];
  const sizes = await Promise.all(PRINTS.map(([style]) => new Promise((ok) => { const im = new Image(); im.onload = () => ok([im.width, im.height]); im.onerror = () => ok(null); im.src = `/promo/.build/long/tex/${P}/${style}.jpg`; })));
  const order = PRINTS.map((p, i) => i).sort((a, b) => ((a * 7919) % 26) - ((b * 7919) % 26));
  for (const [n, k] of order.entries()) {
    const [style, , mode = 0] = PRINTS[k], sz = sizes[k];
    if (!sz) continue;
    const area = 1500 + r() * 900, asp = sz[0] / sz[1];
    const w = Math.sqrt(area * asp), h = w / asp;
    const [ci, cj] = cells[n];
    const p = await print(`/promo/.build/long/tex/${P}/${style}.jpg`, w, h, mode);
    p.g.position.set(center.x + ci * CX + (r() - 0.5) * 12, center.y + cj * CY + (r() - 0.5) * 9, 0.06 + n * 0.003);
    p.g.rotation.z = (r() - 0.5) * 0.12;
    others.push({ ...p, style, cell: [ci, cj] });
  }

  function update(t, { on = 0, show = 0, heroA = 0, heroSelf = 1 }) {
    U.t.value = t;
    U.li.value = on; U.lr.value = 26 + 118 * Math.pow(on, 0.7);
    top.material.uniforms.alpha.value = show;
    hero.m.material.uniforms.alpha.value = heroA; hero.sh.material.uniforms.alpha.value = heroA * show;
    hero.m.material.uniforms.self.value = heroSelf;
    for (const o of others) { o.m.material.uniforms.alpha.value = show; o.sh.material.uniforms.alpha.value = show; }
  }
  // 镜头要对焦的几件（地图坐标）
  const spots = others.map((o) => ({ x: o.g.position.x, y: o.g.position.y, style: o.style }));
  return { group, update, center, spots, hero };
}
