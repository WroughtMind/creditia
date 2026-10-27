// 第 6 段：片尾字幕。灯暗下去，台面上滚起 catnap 真正的片尾字幕（creditia export --piece roll --style cinema，
// 三种版式接成一条，strip.py）；THE END 停在正中，下面那五颗金色菱形变成星号的五瓣；星号让到左边，creditia
// 坐标：地图坐标，字幕平躺在台面上，「往上滚」＝往地图的 +y 走（镜头从正上方往下看，画面上方就是 +y）
import * as THREE from 'three';
import { clamp, prog, smr, esin, eo3, lerp } from '../short/lib.js';
import { tex, petalPts, textCanvas, textMesh } from './text.js';

const GOLD = '#D2B074';
const petalAngle = (i) => Math.PI / 2 - (i * 2 * Math.PI) / 5;

export async function buildCredits({ C, T, center, view, colors }) {
  const group = new THREE.Group();
  const meta = await (await fetch('/promo/.build/long/tex/roll.json')).json();
  const t = await new THREE.TextureLoader().loadAsync('/promo/.build/long/tex/roll.jpg');
  t.colorSpace = THREE.SRGBColorSpace; t.anisotropy = 8;
  const SW = view.h * (meta.w / 1080) * 0.82, u = SW / meta.w, SH = meta.h * u; // 1080p 下字略小于一比一
  const strip = new THREE.Group(); group.add(strip);
  const plane = new THREE.Mesh(new THREE.PlaneGeometry(SW, SH), new THREE.ShaderMaterial({
    uniforms: { map: { value: t }, alpha: { value: 0 } },
    vertexShader: `varying vec2 vUv; void main(){ vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.); }`,
    // 黑底不画，字自己发光一样浮在暗下去的台面上
    fragmentShader: `uniform sampler2D map; uniform float alpha; varying vec2 vUv; void main(){ vec3 c = texture2D(map, vUv).rgb; float k = max(c.r, max(c.g, c.b)); float a = clamp(k * 1.5, 0., 1.); if (a * alpha < .003) discard; gl_FragColor = vec4(c / max(a, .001), a * alpha); }`,
    transparent: true, depthWrite: false,
  }));
  plane.position.set(0, -SH / 2, 0); plane.renderOrder = 40; strip.add(plane);

  // 五颗菱形：从图里抠掉了，这里是单独的网格，跟着字幕走
  const D = meta.diamonds.map((d) => ({ x: d.x * u - SW / 2, y: -d.y * u, s: d.size * u }));
  const dia = D.map((d) => {
    const m = new THREE.Mesh(new THREE.PlaneGeometry(1, 1), new THREE.MeshBasicMaterial({ color: C(GOLD), transparent: true, opacity: 0, depthWrite: false }));
    m.rotation.z = Math.PI / 4; m.scale.setScalar(d.s * 0.72); m.position.set(d.x, d.y, 0.02); m.renderOrder = 41; strip.add(m);
    return m;
  });
  // 五瓣（同 mark.ts），半径 1
  const petals = colors.slice(0, 5).map((c, i) => {
    const pts = petalPts(0, 0, petalAngle(i), 1, 0.224, 0.07);
    const m = new THREE.Mesh(new THREE.ShapeGeometry(new THREE.Shape(pts.map(([x, y]) => new THREE.Vector2(x, y))), 4), new THREE.MeshBasicMaterial({ color: C(c), transparent: true, opacity: 0, depthWrite: false }));
    m.renderOrder = 42; group.add(m); return m;
  });
  const word = textMesh(textCanvas([{ text: 'creditia', color: '#E9E2D3', weight: 600 }], 200, { pad: 0.08 }), view.h * 0.13, 0); group.add(word);
  const url = textMesh(textCanvas([{ text: 'wroughtmind.github.io/creditia', color: '#8F887B', family: 'Mono' }], 96, { pad: 0.05 }), view.h * 0.034, 0); group.add(url);

  // 滚动：字幕顶端从画面下沿进来，THE END 下的菱形停在正中；起步匀速，最后慢慢停住
  const dy = -D[0].y; // 菱形离字幕顶端的距离
  const off0 = center.y - view.h / 2 - 1, off1 = center.y + dy;
  const roll = (k) => { const v0 = 1.25; return v0 * (k * k * k - 2 * k * k + k) + (-2 * k * k * k + 3 * k * k); };
  const R = view.h * 0.085;

  function update(t, { alpha = 1 } = {}) {
    const k = prog(t, T.roll[0], T.roll[1]);
    strip.position.set(center.x, lerp(off0, off1, roll(k)), 0.3);
    const m0 = T.mark[0], m1 = T.mark[1];
    plane.material.uniforms.alpha.value = smr(t, T.roll[0], T.roll[0] + 0.6) * (1 - smr(t, m0 + 0.1, m0 + 1.3)) * alpha;
    // 菱形 → 星号：先朝中心聚，转到各自瓣的方向，金色褪成工具色，长成瓣
    const mk = esin(prog(t, m0, m1));
    const endK = esin(prog(t, T.end[0], T.end[1]));
    const cx = center.x - endK * view.h * 0.27, cy = center.y; // 星号加字标整组居中
    dia.forEach((m, i) => {
      m.material.opacity = smr(t, T.roll[0], T.roll[0] + 0.6) * (1 - smr(t, m0 + 0.35, m0 + 0.9)) * alpha;
      const a = petalAngle(i), tx = cx - strip.position.x + Math.cos(a) * R * 0.5, ty = cy - strip.position.y + Math.sin(a) * R * 0.5;
      m.position.set(lerp(D[i].x, tx, mk), lerp(D[i].y, ty, mk), 0.02);
      m.rotation.z = Math.PI / 4 + (a - Math.PI / 2) * mk;
    });
    petals.forEach((m, i) => {
      const g = clamp((mk - 0.25) / 0.75);
      m.material.opacity = smr(t, m0 + 0.3, m0 + 0.9) * alpha * (1 - smr(t, T.duration - 1.2, T.duration - 0.2));
      m.material.color.copy(C(GOLD)).lerp(C(colors[i]), smr(t, m0 + 0.4, m1));
      m.scale.setScalar(R * (0.35 + 0.65 * eo3(g)) + 0.0001);
      m.position.set(cx, cy, 0.35);
      m.rotation.z = (1 - mk) * 0.6;
    });
    const wk = smr(t, T.end[0] + 0.4, T.end[1] + 0.5);
    word.material.opacity = wk * alpha; word.position.set(cx + R * 1.45 + (1 - eo3(prog(t, T.end[0] + 0.4, T.end[1] + 0.5))) * R * 0.3, cy - R * 0.06, 0.35);
    url.material.opacity = smr(t, T.end[1] + 0.3, T.end[1] + 1.1) * alpha; url.position.set(cx + R * 1.5, cy - R * 1.18, 0.35);
  }
  return { group, update, SH };
}
