// creditia 长片《片尾字幕》：一个项目的 141 天，最后落成一份片尾字幕。render(t) 只依赖 t，同一个 t 永远画出同一帧。
//
//   1 终端：creditia analyze → creditia fetch，镜头钻进 Claude Code 那一格进度条
//   2 地图：catnap 的代码铺在地上，141 天里一段段写出来；时间轴上日期在走，提交一颗颗落下
//   3 证据：推到 pick.ts，每一行标出它凭什么算在谁头上（日志里对得上的、提交里写明的、推断的、未确认的）
//   4 算账：各行的颜色流进份额条，份额条就是署名卡上的那几条
//   5 看样台：灯亮起来，署名卡原来躺在一张台子上，四周摊着同一份署名的 26 种样子
//   6 片尾字幕：灯暗下去，真正的片尾字幕滚起来；THE END 下的五颗菱形变成星号的五瓣，creditia
//
// 空间：地面 y = 0，地图、证据、署名卡、看样台、片尾字幕都在这块地上（地图坐标，见 map.js）；终端竖在地图前方的半空
import * as THREE from 'three';
import { Post } from '../short/post.js';
import { track, lerp, clamp, prog, smr, wob, lin } from '../short/lib.js';
import { T } from './timeline.js';
import { HERO } from './evidence-data.js';
import { buildTerminal } from './terminal.js';
import { buildMap, CAP } from './map.js';
import { buildEvidence } from './evidence.js';
import { buildTally } from './tally.js';
import { buildTable } from './table.js';
import { buildCredits } from './credits.js';

const params = new URLSearchParams(location.search);
const SCALE = Number(params.get('scale') ?? 1);
const W = Math.round(1920 * SCALE), H = Math.round(1080 * SCALE); // 只做横版

const B = '/promo/.build/long/';
const data = await (await fetch(B + 'data.json')).json();
const cardBars = await (await fetch(B + 'tex/card.json')).json();

const renderer = new THREE.WebGLRenderer({ antialias: false, preserveDrawingBuffer: true, powerPreference: 'high-performance' });
renderer.setPixelRatio(1); renderer.setSize(W, H);
renderer.outputColorSpace = THREE.LinearSRGBColorSpace;
document.body.appendChild(renderer.domElement);
const post = new Post(renderer, W, H);
const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(30, W / H, 0.3, 9000);
// 画面的「上」：平视时是 +y，低头看地面时是 -z（地图里文件开头那一边），一个固定的斜向量两种情况都对
camera.up.set(0, 1, -1).normalize();
const C = (hex) => new THREE.Color().setRGB(...lin(hex));
post.clear = C('#0B0A09');

const COLORS = [...data.agents.filter((a) => a.kind === 'agent').map((a) => a.color), '#8A8478', '#CFC8BA']; // 五个工具、未确认、人写的（不上色）
const PCT = data.agents.map((a) => a.pct);

// ───────── 地面上的一切（地图坐标）─────────
const map = buildMap({ data, C, colors: COLORS, heroLines: HERO });
const X0 = -map.mapW / 2, Z0 = -CAP / 2;
const ground = new THREE.Group();
ground.rotation.x = -Math.PI / 2; ground.position.set(X0, 0, Z0);
scene.add(ground);
ground.add(map.group);
const Lw = (x, y, z = 0) => [X0 + x, z, Z0 - y]; // 地图坐标 → 世界坐标

const ev = buildEvidence({ C, T, colors: COLORS });
ground.add(ev.group);
const card = { x: -14, y: -20, w: 64, h: (64 * cardBars.h) / cardBars.w };
const tally = buildTally({ C, T, colors: COLORS, sources: ev.sources, card, bars: cardBars.bars, pct: PCT });
ground.add(tally.group);
const table = await buildTable({ C, card, Lw, lo: SCALE < 0.5 }); // 预览用小贴图（这台机器内存紧）
ground.add(table.group);
const TC = table.center;
const HC = 118, VIEW = { h: 2 * HC * Math.tan((15 * Math.PI) / 180) }; // 片尾字幕时镜头离台面的高度、看得见的高度
const credits = await buildCredits({ C, T, center: TC, view: VIEW, colors: COLORS });
ground.add(credits.group);

// ───────── 终端（地图前方的半空）─────────
const term = buildTerminal({ data, C, T });
const TP = new THREE.Vector3(-12, 258, 236);
term.group.position.copy(TP); scene.add(term.group);
const dive = term.target.clone().add(TP);

// ───────── 镜头：[位置 xyz, 看向 xyz] ─────────
const K = (t, p, l, o = {}) => ({ t, v: [...p, ...l], ...o });
const cam = track([
  K(0, [-1.5, 256.2, TP.z + 30], [-1.5, 256.2, TP.z]),
  K(T.term.out1, [4.6, 255.4, TP.z + 41], [4.6, 255.4, TP.z]),
  K(T.term.out2 + 0.3, [3.2, 251.6, TP.z + 40], [3.2, 251.4, TP.z]),
  K(T.dive[0], [0.4, 250.8, TP.z + 30], [dive.x, dive.y, TP.z]),
  K(T.dive[0] + 1.5, [dive.x, dive.y, TP.z + 2.2], [dive.x, dive.y, TP.z - 30]),
  K(T.dive[1] - 0.9, [dive.x, dive.y - 3, TP.z - 14], [dive.x, dive.y - 70, TP.z - 110]),
  K(T.dive[1], [0, 238, 210], [0, 0, 20]),
  K(14.8, [-70, 330, 300], [-50, 0, 5]),
  K(19.7, Lw(150, -150, 190), Lw(120, -40, 0)),
  K(T.lapse[1], Lw(30, -60, 80), Lw(14, -14, 0)),
  // 3 证据：几乎正上方看 pick.ts；标到面板时斜过去，面板浮在半空的层次出来
  K(T.ev[0], Lw(10, -11.5, 33), Lw(10, -7.5, 0)),
  K(27.8, Lw(20, -14, 45), Lw(20, -6.5, 1)),
  K(31.0, Lw(40, -34, 30), Lw(25, -6, 3)),
  K(34.2, Lw(22, -30, 46), Lw(21, -8, 2)),
  // 4 算账：往后、往下让出份额条和卡片
  K(37.2, Lw(18, -50, 62), Lw(18, -30, 0)),
  K(41.4, Lw(18, -39.5, 72), Lw(TC.x, TC.y, 0), { hold: true }),
  // 5 看样台：压低、贴着台面横移（焦点在一件件作品间换），再升起来看全
  K(46.2, Lw(TC.x, TC.y - 84, 30), Lw(TC.x, TC.y - 8, 2)),
  K(50.8, Lw(TC.x - 92, TC.y - 74, 26), Lw(TC.x - 70, TC.y + 6, 1)),
  K(54.8, Lw(TC.x - 20, TC.y - 60, 250), Lw(TC.x, TC.y, 0)),
  // 6 片尾字幕：正上方，几乎不动
  K(T.roll[0] + 0.2, Lw(TC.x, TC.y - 1.8, HC), Lw(TC.x, TC.y, 0), { hold: true }),
  K(T.duration, Lw(TC.x, TC.y - 1.6, HC * 0.96), Lw(TC.x, TC.y, 0)),
]);

// 日子：时间轴上第几个有提交的日子（0–98），两头缓一点
const dayAt = (t) => { const k = prog(t, T.lapse[0], T.lapse[1]); return (k * k * (3 - 2 * k)) * 0.35 * 98 + k * 0.65 * 98 - (t < T.lapse[0] ? 1 : 0); };

const tmp = new THREE.Vector3();
function render(t) {
  const [px, py, pz, tx, ty, tz] = cam(t);
  camera.position.set(px + wob(t * 0.3, 1) * 0.05, py + wob(t * 0.27, 2) * 0.04, pz);
  camera.lookAt(tx, ty, tz);
  const dist = camera.position.distanceTo(tmp.set(tx, ty, tz));

  // 1 终端：钻进去之前淡掉，免得贴着镜头被近裁剪面切开
  const nearTerm = Math.abs(camera.position.z - TP.z);
  term.group.visible = t < T.dive[1];
  term.update(t, { alpha: clamp((nearTerm - 0.6) / 2.2) * (camera.position.z > TP.z ? 1 : 0) });

  // 2 地图（证据、算账时 pick.ts 开头那几行换成真字；看样台亮灯前退场）
  const heroA = 1 - smr(t, T.ev[0] - 0.6, T.ev[0] + 0.6);
  map.group.visible = t < T.table[0] + 1;
  map.update(dayAt(t), {
    alpha: (1 - 0.55 * smr(t, T.ev[0] - 1, T.ev[0] + 1)) * (1 - smr(t, T.tally[0] + 4, T.table[0])),
    heroA,
    ui: smr(t, T.lapse[0] - 0.4, T.lapse[0] + 0.6) * (1 - smr(t, T.lapse[1] + 0.2, T.lapse[1] + 1.2)),
  });

  // 3 证据
  ev.group.visible = t > T.ev[0] - 1 && t < T.tally[0] + 4;
  ev.update(t, { alpha: 1 - smr(t, T.tally[0] + 0.4, T.tally[0] + 2.4), textA: smr(t, T.ev[0] - 0.6, T.ev[0] + 0.6) * (1 - smr(t, T.tally[0] + 1, T.tally[0] + 3)) });

  // 4 算账
  tally.group.visible = t > T.tally[0] && t < T.tally[0] + 7;
  tally.update(t);

  // 5 看样台：卡片在算账末尾浮出来；灯亮起来时台子和四周的作品才被照见；片尾字幕前灯暗下去
  const on = smr(t, T.table[0], T.table[0] + 3.2) * (1 - smr(t, T.roll[0] - 0.2, T.roll[0] + 1.6));
  table.group.visible = t > tally.landed;
  table.update(t, {
    on, show: smr(t, T.table[0] - 0.4, T.table[0] + 0.4),
    heroA: smr(t, tally.landed + 1.1, tally.landed + 2.3),
    heroSelf: 1 - smr(t, T.table[0], T.table[0] + 1.5),
  });

  // 6 片尾字幕
  credits.group.visible = t > T.roll[0] - 0.5;
  credits.update(t);

  // 镜头：对焦在看着的地方
  const lens = { focus: dist, aperture: lerp(1.1, 0.35, smr(Math.log(dist), Math.log(30), Math.log(400))) * dist * 0.0018 * (t > T.roll[0] ? 0.2 : 1), maxCoc: 14 * SCALE + 4 };
  const fade = smr(t, 0, 0.8) * (1 - smr(t, T.duration - 1.6, T.duration - 0.1));
  post.render(scene, null, camera, lens, { bloom: lerp(0.12, 0.06, on), exposure: 1.0, fade, grain: 0.03, frame: Math.round(t * 60), vignette: 0.42, ca: 0.008, lift: 0.0 });
}

// ───────── 事件表：配乐按它对齐 ─────────
const inv = (d) => { let lo = T.lapse[0], hi = T.lapse[1]; for (let i = 0; i < 40; i++) { const m = (lo + hi) / 2; if (dayAt(m) < d) lo = m; else hi = m; } return +lo.toFixed(3); };
window.EVENTS = {
  duration: T.duration, T,
  keys: term.keys.map((x) => +x.toFixed(3)), lines: term.lines.map((x) => +x.toFixed(3)), enters: term.enters.map((x) => +x.toFixed(3)),
  versions: data.versions.map((v) => { const i = data.days.findIndex((d) => Date.parse(d.date) >= Date.parse(v.date)); return [inv(Math.max(0, i)), v.tag]; }),
  days: data.days.map((d, i) => [inv(i), Object.values(d.by).reduce((a, b) => a + b, 0)]),
  tags: HERO.filter((h) => h.text.trim()).map((h, n) => [+(T.ev[0] + 1.2 + n * 0.36).toFixed(3), h.cls, h.kind]),
  tally: tally.landed,
};
window.render = render;
window.STATS = { strokes: map.strokes, ticks: map.ticks, mapW: map.mapW };
render(0);
window.READY = true;
