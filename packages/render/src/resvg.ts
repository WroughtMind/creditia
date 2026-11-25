// Catalog and text exports can use the Node renderer without a native addon.
import { createRequire } from 'node:module';
import type { ResvgRenderOptions } from '@resvg/resvg-js';

const require = createRequire(import.meta.url);
let Resvg: typeof import('@resvg/resvg-js').Resvg;

export function resvg(svg: string, options?: ResvgRenderOptions) {
  Resvg ??= require('@resvg/resvg-js').Resvg;
  return new Resvg(svg, options);
}
