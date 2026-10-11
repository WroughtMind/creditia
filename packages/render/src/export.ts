import { PDFDocument } from 'pdf-lib';
import gifenc from 'gifenc';
const { GIFEncoder, quantize, applyPalette } = gifenc;

export type ExportFormat = 'png' | 'svg' | 'pdf' | 'gif' | 'html';
export interface Raster { png: Uint8Array; rgba: Uint8Array; width: number; height: number }
export type Rasterize = (svg: string, width?: number) => Promise<Raster> | Raster;
export const EXPORTS: { id: ExportFormat; label: string; description: string; mime: string }[] = [
  { id: 'png', label: 'PNG', description: '图片 · 社交与分享', mime: 'image/png' },
  { id: 'svg', label: 'SVG', description: '矢量 · 网站与设计', mime: 'image/svg+xml' },
  { id: 'pdf', label: 'PDF', description: '文档 · 演示与打印', mime: 'application/pdf' },
  { id: 'gif', label: 'GIF', description: '动图 · 字幕与动态展示', mime: 'image/gif' },
  { id: 'html', label: 'HTML', description: '独立网页 · 离线可打开', mime: 'text/html' },
];
export function dimensions(svg: string) {
  const tag = svg.match(/<svg\b[^>]*>/)?.[0] ?? '';
  const width = Number(tag.match(/\bwidth="([\d.]+)"/)?.[1]);
  const height = Number(tag.match(/\bheight="([\d.]+)"/)?.[1]);
  if (!(width > 0 && height > 0)) throw new Error('作品缺少有效尺寸');
  return { width, height };
}
const escapeHtml = (text: string) => text.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);

/** 同一份帧定义用于 GIF；HTML 与 App 用等价的平移动画。 */
export function motionSpec(svg: string, form: string) {
  const { width, height } = dimensions(svg);
  return { width, height, viewport: height, travel: form === 'roll' ? height * 0.3 : 0 };
}
export function frameSvg(svg: string, form: string, progress: number, background: string) {
  const { width, viewport, travel } = motionSpec(svg, form);
  const inside = svg.replace(/^.*?<svg\b[^>]*>/s, '').replace(/<\/svg>\s*$/, '');
  const p = progress < 0.12 ? 0 : progress > 0.86 ? 1 : (progress - 0.12) / 0.74;
  const opacity = travel ? 1 : 0.72 + 0.28 * Math.sin(Math.PI * progress);
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${viewport}" viewBox="0 0 ${width} ${viewport}"><rect width="100%" height="100%" fill="${background}"/><g opacity="${opacity}" transform="translate(0 ${-travel * p})">${inside}</g></svg>`;
}
export function standaloneHtml(svg: string, title: string, form: string, background: string) {
  const { width, height, viewport, travel } = motionSpec(svg, form);
  return `<!doctype html><html lang="zh"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${escapeHtml(title)}</title><style>*{box-sizing:border-box}body{margin:0;background:${background};display:grid;place-items:center;min-height:100svh}figure{margin:0;width:100%;max-width:${width}px;aspect-ratio:${width}/${viewport};overflow:hidden}svg{display:block;width:100%;height:auto;${travel ? `animation:credits 8s linear infinite;` : ''}}@keyframes credits{0%,12%{transform:translateY(0)}86%,100%{transform:translateY(-${travel / height * 100}%)} }figure:hover svg{animation-play-state:paused}@media(prefers-reduced-motion:reduce){svg{animation:none}figure{aspect-ratio:${width}/${height}}}@media print{body{min-height:0}figure{aspect-ratio:${width}/${height}}svg{animation:none}}</style></head><body><figure aria-label="${escapeHtml(title)}">${svg}</figure></body></html>`;
}
export async function encodeExport(svg: string, format: ExportFormat, rasterize: Rasterize, options: { title: string; form: string; background: string; width?: number; progress?: (value: number) => void }): Promise<Uint8Array> {
  const text = new TextEncoder();
  if (format === 'svg') return text.encode(svg);
  if (format === 'html') return text.encode(standaloneHtml(svg, options.title, options.form, options.background));
  if (format === 'png') return (await rasterize(svg, options.width)).png;
  if (format === 'pdf') {
    const raster = await rasterize(svg, options.width);
    const doc = await PDFDocument.create();
    doc.setTitle(options.title); doc.setCreator('creditia');
    const picture = await doc.embedPng(raster.png);
    const size = dimensions(svg);
    const page = doc.addPage([size.width * 0.75, size.height * 0.75]);
    page.drawImage(picture, { x: 0, y: 0, width: page.getWidth(), height: page.getHeight() });
    return doc.save();
  }
  if (format === 'gif') {
    const gif = GIFEncoder();
    // 8 秒、12 fps 是动图媒介的初始节奏，不截断数据或限制用户的输出尺寸。
    const frames = 96;
    for (let i = 0; i < frames; i++) {
      const raster = await rasterize(frameSvg(svg, options.form, i / (frames - 1), options.background), options.width);
      const palette = quantize(raster.rgba, 256);
      gif.writeFrame(applyPalette(raster.rgba, palette), raster.width, raster.height, { palette, delay: 1000 / 12, repeat: 0 });
      options.progress?.((i + 1) / frames);
      await new Promise(resolve => setTimeout(resolve, 0));
    }
    gif.finish();
    return gif.bytes();
  }
  throw new Error(`不支持的导出格式：${format}`);
}
