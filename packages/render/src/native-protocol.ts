// Node adapter only: the protocol itself does not read the filesystem.
import { deflateSync } from 'node:zlib';
import { raster as svgRaster } from './index.ts';
import { art, frames, raster as classicRaster } from './classic/index.ts';
import { dispatchRender, type RenderBackend } from './protocol.ts';

import { embeddedFonts } from './embed-fonts.ts';

// Preserve the CLI's raster options for already outlined classic artwork while
// loading fonts for SVG pieces (including those borrowed by classic themes).
const rasterFor = (svg: string) => svg.includes('<text') ? svgRaster : classicRaster;
export const nativeBackend: RenderBackend = {
  raster: {
    png: (svg, width) => rasterFor(svg).png(svg, width),
    rgba: (svg, width) => rasterFor(svg).rgba(svg, width),
    flat: svg => rasterFor(svg).flat(svg),
    embed: embeddedFonts,
  },
  deflate: bytes => new Uint8Array(deflateSync(bytes)),
  classicArt: art,
  classicFrames: frames,
};

export const renderRequest = (request: unknown) => dispatchRender(request, nativeBackend);
