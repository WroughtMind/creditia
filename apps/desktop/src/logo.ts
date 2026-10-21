// creditia 标志，设置里可以换，程序坞图标跟着换（src-tauri 的 set_logo）。横纹都是工具颜色（Claude Code、Codex、Cursor、Kimi、Grok、未确认）。
// orb：彩色横纹的圆，星从右上亮出来（默认，也是安装包图标）
// ring：六道胶囊围成一圈，星在正中
// color / mono：最早的标志，横纹组成的 C，开口处一颗星；黑白版整个跟随 currentColor
import orb from '../../../packages/render/assets/creditia/logo-orb.svg?raw';
import ring from '../../../packages/render/assets/creditia/logo-ring.svg?raw';
import color from '../../../packages/render/assets/creditia/logo-color.svg?raw';
import mono from '../../../packages/render/assets/creditia/logo.svg?raw';

const inline = (svg: string) => svg.trim().replaceAll('#0E0E0E', 'currentColor').replace('<svg ', '<svg aria-hidden="true" ');
export const DEFAULT_LOGO = 'orb';
export const LOGOS: Record<string, { svg: string; name: [string, string] }> = {
  orb: { svg: inline(orb), name: ['条纹圆', 'Orb'] },
  ring: { svg: inline(ring), name: ['光环', 'Ring'] },
  color: { svg: inline(color), name: ['C 字', 'C mark'] },
  mono: { svg: inline(mono), name: ['C 字 · 黑白', 'C mark, mono'] },
};
export const logoSvg = (id: string | null) => (LOGOS[id ?? ''] ?? LOGOS[DEFAULT_LOGO]).svg;
