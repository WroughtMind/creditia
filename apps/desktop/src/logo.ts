// creditia 标志：由片尾字幕的横行组成的 C，开口处一颗四角星（AI）。
// 彩色版的六道横纹是工具颜色（Claude Code、Codex、Cursor、Kimi、Grok、未确认），和宣传片结尾一样；黑白版整个跟随 currentColor。
// 设置里可以换，程序坞图标跟着换（src-tauri 的 set_logo）。
import color from '../../../packages/render/assets/creditia/logo-color.svg?raw';
import mono from '../../../packages/render/assets/creditia/logo.svg?raw';

const inline = (svg: string) => svg.trim().replaceAll('#0E0E0E', 'currentColor').replace('<svg ', '<svg aria-hidden="true" ');
export const LOGOS: Record<string, { svg: string; name: [string, string] }> = {
  color: { svg: inline(color), name: ['彩色', 'Color'] },
  mono: { svg: inline(mono), name: ['黑白', 'Mono'] },
};
export const logoSvg = (id: string | null) => (LOGOS[id ?? ''] ?? LOGOS.color).svg;
