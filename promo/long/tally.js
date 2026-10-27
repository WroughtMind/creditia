// 第 4 段：算账。各行的颜色（还有整个仓库里别的行）化成一颗颗小星号，流进份额条；
// 份额条的位置和长短就是署名卡上那六条（cardbars.py 从卡片图里量出来的），卡片浮出来时正好接上
import * as THREE from 'three';
import { rng, clamp, smr } from '../short/lib.js';
import { tex, starCanvas } from './text.js';

export function buildTally({ C, T, colors, sources, card, bars, pct }) {
  const group = new THREE.Group();
  const r = rng(77);
  const N = 1100;
  // 每条在地图坐标里的位置
  const B = bars.map((b, i) => ({
    x0: card.x + b.x0 * card.w, x1: card.x + b.fill * card.w, y: card.y - b.y * card.h, h: Math.max(0.22, b.h * card.h), cls: i,
  }));
  const cx = card.x + card.w * 0.7, cy = card.y - card.h * 0.5;
  const P = [];
  const total = pct.reduce((a, b) => a + b, 0);
  pct.forEach((p, cls) => {
    const n = Math.max(6, Math.round((p / total) * N));
    const mine = sources.filter((s) => s.cls === cls);
    for (let k = 0; k < n; k++) {
      let sx, sy, delay;
      if (mine.length && r() < 0.55) { const s = mine[Math.floor(r() * mine.length)]; sx = s.x + (r() - 0.5) * 1.2; sy = s.y + (r() - 0.5) * 0.6; delay = 0.25 + r() * 1.4; }
      else { const a = r() * Math.PI * 2, d = 70 + r() * 90; sx = cx + Math.cos(a) * d; sy = cy + Math.sin(a) * d * 0.7; delay = r() * 1.6; } // 镜头外，整个仓库里别的行
      const b = B[cls];
      const tx = b.x0 + r() * Math.max(0.05, b.x1 - b.x0), ty = b.y + (r() - 0.5) * b.h * 0.6;
      P.push([sx, sy, tx, ty, T.tally[0] + delay, 1.25 + r() * 0.55, 3 + r() * 7, cls]);
    }
  });
  const quad = new THREE.PlaneGeometry(1, 1);
  const geo = new THREE.InstancedBufferGeometry();
  geo.index = quad.index; geo.setAttribute('position', quad.getAttribute('position')); geo.setAttribute('uv', quad.getAttribute('uv'));
  geo.setAttribute('p0', new THREE.InstancedBufferAttribute(new Float32Array(P.flatMap((p) => [p[0], p[1], p[2], p[3]])), 4));
  geo.setAttribute('p1', new THREE.InstancedBufferAttribute(new Float32Array(P.flatMap((p) => [p[4], p[5], p[6], p[7]])), 4));
  geo.instanceCount = P.length;
  const mat = new THREE.ShaderMaterial({
    uniforms: { t: { value: 0 }, alpha: { value: 1 }, map: { value: tex(starCanvas(64)) }, pal: { value: colors.map((c) => C(c)) }, size: { value: 0.42 } },
    vertexShader: `attribute vec4 p0, p1; uniform float t, size; uniform vec3 pal[7]; varying vec2 vUv; varying vec3 vC; varying float vA;
      void main(){
        vUv = uv;
        float k = clamp((t - p1.x) / p1.y, 0., 1.);
        float e = k * k * (3. - 2. * k);
        vec2 p = mix(p0.xy, p0.zw, e);
        float z = sin(3.14159 * e) * p1.z + .25;
        vec3 col = pal[0]; for (int i = 0; i < 7; i++) if (i == int(p1.w)) col = pal[i];
        vC = col * (1.5 + 1.2 * sin(3.14159 * e)); vA = step(.0001, t - p1.x);
        float s = size * (1. + 1.4 * sin(3.14159 * e)) * mix(1., .55, step(1., k));
        gl_Position = projectionMatrix * modelViewMatrix * vec4(p + position.xy * s, z, 1.);
      }`,
    fragmentShader: `uniform sampler2D map; uniform float alpha; varying vec2 vUv; varying vec3 vC; varying float vA;
      void main(){ float a = texture2D(map, vUv).a * vA * alpha; if (a < .01) discard; gl_FragColor = vec4(vC, a); }`,
    transparent: true, depthWrite: false,
  });
  const mesh = new THREE.Mesh(geo, mat); mesh.frustumCulled = false; mesh.renderOrder = 30; group.add(mesh);

  // 实心的条：星号落满后长出来，卡片浮出后让给卡片自己的条
  const solid = B.map((b) => {
    const m = new THREE.Mesh(new THREE.PlaneGeometry(1, 1), new THREE.MeshBasicMaterial({ color: C(colors[b.cls]).multiplyScalar(1.1), transparent: true, opacity: 0, depthWrite: false }));
    m.scale.set(Math.max(0.05, b.x1 - b.x0), b.h, 1); m.position.set((b.x0 + b.x1) / 2, b.y, 0.22); m.renderOrder = 29; group.add(m);
    return m;
  });
  const landed = T.tally[0] + 3.2;
  function update(t) {
    mat.uniforms.t.value = t;
    mat.uniforms.alpha.value = 1 - smr(t, landed + 0.3, landed + 1.1);
    const sa = smr(t, landed, landed + 0.6) * (1 - smr(t, landed + 1.6, landed + 2.4));
    solid.forEach((m) => (m.material.opacity = sa));
  }
  return { group, update, landed, bars: B };
}
