import { el, rect, path, r, type Scene, type Attrs } from './svg.ts';
import type { Box } from './style.ts';
import { phase } from './motion.ts';

type PaperKind = 'ink' | 'ukiyoe' | 'woodblock' | 'baroque' | 'grimoire' | 'manuscript';
type Bounds = Pick<Box, 'x' | 'y' | 'w' | 'h'>;
const sequence = new WeakMap<Scene, number>();
const name = (s: Scene, kind: string) => {
  const n = sequence.get(s) ?? 0; sequence.set(s, n + 1); return `paper-${kind}-${n}`;
};

/** 完整字面/印面的着色过程。底墨始终连续，纹理不把细字、真份额切成缺段。 */
export function paperSurface(s: Scene, kind: PaperKind, k: number, b: Bounds, text: boolean, valueAxis?: 'x' | 'y'): Attrs {
  const rest = 1 - k;
  if (kind === 'baroque') {
    // 浮饰只在真实字形/金面内部起棱，外廓和归属色保持不动；不是另加金色轮廓。
    const edge = Math.min(text ? .65 : 2.1, Math.max(.25, Math.min(b.w, b.h) * .09)) * rest;
    const filter = s.url(name(s, kind), id => el('filter', { id, filterUnits: 'userSpaceOnUse', x: b.x - 4, y: b.y - 4, width: b.w + 8, height: b.h + 8 },
      el('feMorphology', { in: 'SourceAlpha', operator: 'erode', radius: edge, result: 'inner' }),
      el('feComposite', { in: 'SourceAlpha', in2: 'inner', operator: 'out', result: 'rim' }),
      el('feComponentTransfer', { in: 'SourceGraphic', result: 'raised' },
        ...['R', 'G', 'B'].map(c => el(`feFunc${c}`, { type: 'gamma', amplitude: 1, exponent: .55 + .45 * k, offset: 0 }))),
      el('feComposite', { in: 'raised', in2: 'rim', operator: 'in', result: 'edge' }),
      el('feComponentTransfer', { in: 'SourceGraphic', result: 'face' }, el('feFuncA', { type: 'linear', slope: .48 + .52 * phase(k, .12, .86) })),
      el('feMerge', {}, el('feMergeNode', { in: 'face' }), el('feMergeNode', { in: 'edge' }))));
    return { opacity: phase(k, 0, .12), filter };
  }

  const base = text ? kind === 'manuscript' ? .92 : .76 : kind === 'woodblock' ? .67 : kind === 'grimoire' ? .58 : kind === 'manuscript' ? .76 : .48;
  const floor = base + (1 - base) * phase(k, .58, .98);
  let surface = '';
  if (kind === 'ink') {
    // 浓墨在完整淡墨字面上洇合。弯曲的湿边只改变局部浓度，不裁断字或数据。
    const vertical = valueAxis !== 'y';
    const width = vertical ? b.w : b.h, height = vertical ? b.h : b.w;
    const wet = Array.from({ length: 15 }, (_, i) => {
      const x = width * i / 14;
      const y = height * (1 - k) + Math.sin(i * 1.35) * Math.min(7, height * .12) * Math.sin(Math.PI * k);
      return `${i ? 'L' : 'M'}${r(x)} ${r(y)}`;
    }).join('');
    const blur = s.url(name(s, 'wet'), id => el('filter', { id, x: '-30%', y: '-40%', width: '160%', height: '180%' }, el('feGaussianBlur', { stdDeviation: Math.min(4, Math.max(.7, height * .08)) })));
    surface = path(`${wet}L${width} ${height + 8}H0Z`, { fill: '#fff', filter: blur, transform: vertical ? `translate(${b.x} ${b.y})` : `translate(${b.x} ${b.y}) matrix(0 1 1 0 0 0)` });
  } else if (kind === 'ukiyoe') {
    // 整张色版的印压合实；量条只沿厚度改变接触浓度，完整底版一直可辨。
    // 格图交出整张版的 bounds，不能在每个格子里造一个无关圆斑。
    const pressure = s.url(name(s, 'baren'), id => el('linearGradient', { id, x1: 0, y1: 0, x2: valueAxis === 'y' ? 1 : 0, y2: valueAxis === 'y' ? 0 : 1 },
      el('stop', { offset: Math.max(0, k - .16), stopColor: '#fff' }),
      el('stop', { offset: Math.min(1, k + .22), stopColor: '#fff', stopOpacity: 0 })));
    surface = rect(b.x, b.y, b.w, b.h, { fill: pressure });
  } else {
    let w = 16, h = 16, marks = '';
    if (kind === 'woodblock') {
      // 完整底墨上的细刀痕，宽度不随帧扩成条码；不规则断续刀路逐渐吃墨。
      w = 17; h = 31;
      const cut = .9 * rest;
      marks = rect(0, 0, w, h, { fill: '#fff' })
        + path('M2 0L2.4 8L1.8 16M7.5 9L8 23L7.4 31M13 0L12.6 11M13.2 18L12.8 31', { fill: 'none', stroke: '#000', strokeWidth: cut, strokeOpacity: .7 * rest });
    } else if (kind === 'grimoire') {
      // 古籍的两向细排线在完整底墨里交织，避免退回 mono 的单一轮廓。
      w = h = 15;
      marks = path('M-15 0L15 30M0 -15L30 15', { fill: 'none', stroke: '#fff', strokeWidth: 1.2 })
        + path('M-15 15L15 -15M0 30L30 0', { fill: 'none', stroke: '#fff', strokeWidth: 1, strokeOpacity: phase(k, .1, .55) });
    } else {
      // 手稿图形保留整面淡稿，单向短笔触逐渐接实；文本的校注另按真实字面定位。
      w = 23; h = 19;
      marks = path('M1 4L5 2M10 8L14 5M19 2L22 1M3 16L7 14M14 18L18 15M20 11L23 9', {
        fill: 'none', stroke: '#fff', strokeWidth: .6 + k * 4, strokeLinecap: 'round',
      });
    }
    const texture = s.url(name(s, `${kind}-grain`), id => el('pattern', { id, patternUnits: 'userSpaceOnUse', width: w, height: h }, marks));
    surface = rect(b.x - 4, b.y - 4, b.w + 8, b.h + 8, { fill: texture });
  }
  const mask = s.url(name(s, `${kind}-face`), id => el('mask', { id, maskUnits: 'userSpaceOnUse', x: b.x - 4, y: b.y - 4, width: b.w + 8, height: b.h + 8 },
    rect(b.x - 4, b.y - 4, b.w + 8, b.h + 8, { fill: '#fff', opacity: floor }), surface));
  const filter = text && (kind === 'woodblock' || kind === 'ink') ? s.url(name(s, 'type-pressure'), id => el('filter', {
    id, filterUnits: 'userSpaceOnUse', x: b.x - 4, y: b.y - 4, width: b.w + 8, height: b.h + 8,
  }, el('feMorphology', { operator: 'dilate', radius: (kind === 'woodblock' ? .55 : .22) * rest }),
  kind === 'ink' && el('feGaussianBlur', { stdDeviation: .6 * rest }))) : undefined;
  return { opacity: phase(k, 0, .12), mask, filter };
}
