// App 字体：npm run fonts 生成的子集。展示用 @font-face（浏览器按需加载），导出用 resvg-wasm（按风格取字节）。
import manifest from './fonts/manifest.json';
import { familiesIn } from '../../../packages/render/src/fonts.ts';
import type { Style } from '../../../packages/render/src/style.ts';

const urls = import.meta.glob('./fonts/*.ttf', { query: '?url', import: 'default', eager: true }) as Record<string, string>;
const urlOf = (file: string) => urls[`./fonts/${file}`];

/** 注册全部 @font-face；浏览器只在用到时下载 */
export function registerFonts() {
  const css = (manifest as any[]).map((f) => `@font-face{font-family:"${f.family}";font-weight:${f.weight};font-style:${f.italic ? 'italic' : 'normal'};font-display:swap;src:url("${urlOf(f.file)}") format("truetype")}`).join('\n');
  const el = document.createElement('style');
  el.textContent = css;
  document.head.appendChild(el);
}

/** 等某个风格要用的字体加载完，免得先闪一下后备字体 */
export async function ready(st: Style) {
  const fams = new Set([st.f.display, st.f.body, st.f.num, st.f.label, st.f.mono, st.f.script].filter(Boolean).flatMap((f) => familiesIn(f!.family)));
  await Promise.all([...fams].map((f) => document.fonts.load(`16px "${f}"`).catch(() => null)));
}

const bytes = new Map<string, Promise<Uint8Array>>();
/** 导出要用的字体字节：只取风格用到的字体与字重，加中文后备的常规、粗体两档（每个中文字体子集都有几 MB，全载会撑爆内存） */
export async function fontBytes(st: Style, svg = ''): Promise<Uint8Array[]> {
  const specs = [st.f.display, st.f.body, st.f.num, st.f.label, st.f.mono, st.f.script].filter(Boolean) as { family: string; weight?: number; italic?: boolean }[];
  const want = new Map<string, Set<number>>();
  const add = (fam: string, w: number) => { if (!want.has(fam)) want.set(fam, new Set()); want.get(fam)!.add(w); };
  for (const f of specs) for (const fam of familiesIn(f.family)) add(fam, f.weight ?? 400);
  // 作品里实际出现的字体（印章、拟声字、署名块……）
  for (const t of svg.matchAll(/<text\b[^>]*>/g)) {
    const fam = /font-family="([^"]+)"/.exec(t[0])?.[1];
    const w = Number(/font-weight="(\d+)"/.exec(t[0])?.[1] ?? 400);
    if (fam) for (const f of familiesIn(fam)) add(f, w);
  }
  for (const fam of ['Noto Sans SC', 'Noto Serif SC']) { add(fam, 400); add(fam, 700); }
  add('Inter', 400); add('Inter', 600);
  const all = manifest as any[];
  const pick: any[] = [];
  for (const [fam, ws] of want) {
    const faces = all.filter((f) => f.family === fam && !f.italic);
    for (const w of ws) {
      const best = faces.sort((a, b) => Math.abs(a.weight - w) - Math.abs(b.weight - w))[0];
      if (best && !pick.includes(best)) pick.push(best);
    }
  }
  return Promise.all(pick.map((f) => {
    if (!bytes.has(f.file)) bytes.set(f.file, fetch(urlOf(f.file)).then((r) => r.arrayBuffer()).then((b) => new Uint8Array(b)));
    return bytes.get(f.file)!;
  }));
}

/** 原版渲染器（satori）要的字体文件地址：按需取，不注册 @font-face */
export const fontFiles = () => (manifest as any[]).filter((f) => !f.italic).map((f) => ({ family: f.family as string, weight: f.weight as number, url: urlOf(f.file) }));
