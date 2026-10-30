import { prepare, render as classicArt, frames as classicFrames } from '../../src/classic/browser.ts';
import { buildModel as classicModel } from '../../src/classic/data.ts';
import { themeById } from '../../src/classic/themes.ts';
import { art, frames } from '../../src/registry.ts';
import { buildModel } from '../../src/data.ts';
import { styleById } from '../../src/styles/index.ts';

let raw: any, modern: any, classic: any;
self.onmessage = async ({ data }) => {
  if (data.init) {
    raw = data.raw;
    modern = buildModel(raw, { lang: 'zh', sign: '', named: true });
    classic = classicModel(raw, { lang: 'zh', director: false, named: true });
    await prepare(data.fonts);
    return;
  }
  const { seq, style, id, variant, original } = data;
  try {
    const ctx: any = { lang: 'zh', variant, projects: [classic], person: classic.director };
    // 溯源模板需要单行记录；明确作为演示记录，不能冒充 catnap 的实机归因。
    if (id === 'blame') ctx.blame = { file: 'example.ts', line: 1, name: '演示记录', commit: 'demo', subject: '溯源版式演示', evidence: 'unattributed' };
    const still = original ? await classicArt(id, classic, themeById(style)!, ctx) : art(id, modern, styleById(style)!, { variant });
    if (!still) { self.postMessage({ seq, empty: true }); return; }
    self.postMessage({ seq, still });
    const animation = original ? await classicFrames(id, classic, themeById(style)!, ctx, 24) : frames(id, modern, styleById(style)!, { variant }, 24);
    self.postMessage({ seq, animation });
  } catch (error) { self.postMessage({ seq, error: error instanceof Error ? error.message : String(error) }); }
};
