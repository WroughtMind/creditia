// Node adapter only: the protocol itself does not read the filesystem.
import { deflateSync } from 'node:zlib';
import { raster } from './index.ts';
import { art, frames } from './classic/index.ts';
import { dispatchRender, type RenderBackend } from './protocol.ts';

export const nativeBackend: RenderBackend = {
  // Includes classic bridge fonts as well as SVG style fonts. The classic-only
  // raster would drop text in pieces borrowed from the SVG renderer.
  raster,
  deflate: bytes => new Uint8Array(deflateSync(bytes)),
  classicArt: art,
  classicFrames: frames,
};

export const renderRequest = (request: unknown) => dispatchRender(request, nativeBackend);
