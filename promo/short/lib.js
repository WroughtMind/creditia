// 小工具：缓动、随机、插值、相机关键帧。一切只依赖 t，同一个 t 永远画出同一帧。
export const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
export const lerp = (a, b, k) => a + (b - a) * k;
export const prog = (t, a, b) => clamp((t - a) / (b - a));
export const sm = (k) => k * k * (3 - 2 * k);
export const smr = (t, a, b) => sm(prog(t, a, b));
export const eo2 = (k) => 1 - (1 - k) * (1 - k);
export const eo3 = (k) => 1 - Math.pow(1 - k, 3);
export const eio2 = (k) => (k < 0.5 ? 2 * k * k : 1 - Math.pow(-2 * k + 2, 2) / 2);
export const eio3 = (k) => (k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2);
export const eio5 = (k) => (k < 0.5 ? 16 * k ** 5 : 1 - Math.pow(-2 * k + 2, 5) / 2);
export const eoExpo = (k) => (k >= 1 ? 1 : 1 - Math.pow(2, -10 * k));
// 更柔和的 S 曲线：sin 型，起止速度都为 0
export const esin = (k) => 0.5 - 0.5 * Math.cos(Math.PI * clamp(k));

export function rng(seed) {
  return () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
export const hash1 = (n) => { const s = Math.sin(n * 127.1 + 311.7) * 43758.5453; return s - Math.floor(s); };

// 平滑的一维噪声（几组不可公约的正弦叠加），用于相机呼吸与线条漂移
export function wob(t, seed = 0) {
  return (Math.sin(t * 0.61 + seed * 1.7) * 0.5 + Math.sin(t * 0.37 + seed * 3.1 + 1.3) * 0.3 + Math.sin(t * 1.13 + seed * 0.7 + 2.1) * 0.2);
}

export function hex(c) { return [parseInt(c.slice(1, 3), 16) / 255, parseInt(c.slice(3, 5), 16) / 255, parseInt(c.slice(5, 7), 16) / 255]; }
// sRGB → 线性
export function lin(c) { return hex(c).map((v) => (v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4))); }

// 关键帧轨道：[{t, v:[...]}]，非均匀 Catmull-Rom（三次 Hermite），首尾速度为 0；key.hold 让该点速度为 0
export function track(keys) {
  const n = keys.length, dim = keys[0].v.length;
  const m = keys.map((k, i) => {
    if (i === 0 || i === n - 1 || k.hold) return new Array(dim).fill(0);
    const a = keys[i - 1], b = keys[i + 1];
    return k.v.map((_, d) => (b.v[d] - a.v[d]) / (b.t - a.t));
  });
  return (t) => {
    if (t <= keys[0].t) return keys[0].v.slice();
    if (t >= keys[n - 1].t) return keys[n - 1].v.slice();
    let i = 0; while (keys[i + 1].t < t) i++;
    const a = keys[i], b = keys[i + 1], h = b.t - a.t;
    let s = (t - a.t) / h;
    if (a.ease) s = a.ease(s);
    const s2 = s * s, s3 = s2 * s;
    const h00 = 2 * s3 - 3 * s2 + 1, h10 = s3 - 2 * s2 + s, h01 = -2 * s3 + 3 * s2, h11 = s3 - s2;
    return a.v.map((_, d) => h00 * a.v[d] + h10 * h * m[i][d] + h01 * b.v[d] + h11 * h * m[i + 1][d]);
  };
}

// GLSL 公共片段
export const GLSL_NOISE = /* glsl */ `
float hash12(vec2 p){ vec3 p3 = fract(vec3(p.xyx) * .1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y) * p3.z); }
float vnoise(vec2 p){ vec2 i = floor(p), f = fract(p); vec2 u = f*f*(3.-2.*f);
  return mix(mix(hash12(i), hash12(i+vec2(1,0)), u.x), mix(hash12(i+vec2(0,1)), hash12(i+vec2(1,1)), u.x), u.y); }
float fbm(vec2 p){ float s = 0., a = .5; for (int i = 0; i < 5; i++) { s += a * vnoise(p); p = p * 2.03 + 17.1; a *= .5; } return s; }
`;
