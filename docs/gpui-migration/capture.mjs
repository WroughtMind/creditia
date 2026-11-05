#!/usr/bin/env node
// Source inventory only. This does not render artwork or assert GPUI parity.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import vm from 'node:vm';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../..');
const baseline = 'ac82a574b0747bc5651571a3f169f4a893442f88';
const seen = new Map();
function read(file) {
  if (!seen.has(file)) seen.set(file, fs.readFileSync(path.join(root, file), 'utf8'));
  return seen.get(file);
}
function at(file, anchor) {
  const source = read(file), offset = source.indexOf(anchor);
  if (offset < 0) throw new Error(`Missing source anchor ${file}: ${anchor}`);
  return { path: file, line: source.slice(0, offset).split('\n').length, anchor };
}
function value(file, name, context = {}) {
  const source = read(file);
  const match = new RegExp(`(?:export )?const ${name}(?:\\s*:[^=]+)?\\s*=\\s*([\\s\\S]*?);`).exec(source);
  if (!match) throw new Error(`Missing declaration ${file}: ${name}`);
  // Only selected declarative catalog constants, never a complete source module.
  return vm.runInNewContext(`(${match[1].replace(/\s+as const\s*$/, '')})`, context, { timeout: 1000 });
}
const uniq = values => [...new Set(values)];
const main = 'apps/desktop/src/main.ts';
const kitFile = 'apps/desktop/src/kits.ts';
const registry = 'packages/render/src/registry.ts';
const catalog = 'packages/render/src/classic/catalog.ts';
const formsRegistry = 'packages/render/src/classic/registry.ts';
const forms = 'packages/render/src/classic/forms.ts';
const bridge = 'packages/render/src/classic/bridge.ts';
const stylesIndex = 'packages/render/src/styles/index.ts';
const exportFile = 'packages/render/src/export.ts';
const classicBoard = value(kitFile, 'CLASSIC_BOARD');
const v2Board = value(kitFile, 'V2_BOARD');
const classicNames = value(catalog, 'THEME_INFO');
const classicOrder = value(catalog, 'THEME_ORDER');
const skipped = [...value(kitFile, 'SKIP')];

const arrays = {};
for (const [name, file] of [
  ['TIER_VARIANTS', 'tier'], ['ROLL_VARIANTS', 'roll'], ['DONUT_VARIANTS', 'charts'],
  ['WAFFLE_VARIANTS', 'charts'], ['RECEIPT_VARIANTS', 'cards'], ['BADGE_VARIANTS', 'cards'], ['BYLINE_VARIANTS', 'cards'],
]) arrays[name] = value(`packages/render/src/pieces/${file}.ts`, name);
const extras = value(bridge, 'BRIDGED_LAYOUTS', arrays);
const bridged = value(bridge, 'BRIDGED');
const classicVariants = {
  roll: [...value(forms, 'ROLL_VARIANTS'), ...value('packages/render/src/classic/rolls.ts', 'ROLL_NEW')],
  receipt: value(forms, 'RECEIPT_VARIANTS'), app: value(forms, 'APP_VARIANTS'),
};
const defaults = { roll: value(forms, 'ROLL_DEFAULT'), receipt: value(forms, 'RECEIPT_DEFAULT') };
const variantNames = value(registry, 'VARIANT_NAMES');
const classicVariantNames = { ...variantNames, ...value(catalog, 'VARIANT_INFO') };
const pieceInfo = value(catalog, 'PIECE_INFO');

function entries(file) {
  return [...read(file).matchAll(/^  (\w+): \{ (.*)$/gm)].filter(m => /render:/.test(m[2]));
}
const v2Pieces = entries(registry).map(m => {
  const variants = /variants: (\w+)/.exec(m[2]);
  return {
    id: m[1], name: /name: '([^']+)'/.exec(m[2])[1],
    variants: variants ? arrays[variants[1]] : [], animated: m[2].includes('animated: true'),
    desktop: v2Board.some(x => x.id === m[1]), source: at(registry, m[0]),
  };
});
const formEntries = entries(formsRegistry);
const classicPieces = uniq([...formEntries.map(m => m[1]), 'badge', ...bridged]).map(id => {
  const own = classicVariants[id] ?? (extras[id]?.length ? ['original'] : []);
  const variants = [...own, ...(extras[id] ?? [])];
  const sourceEntry = formEntries.find(m => m[1] === id);
  const v2 = v2Pieces.find(p => p.id === id);
  return {
    id, name: pieceInfo[id]?.zh ?? v2?.name ?? /name: '([^']+)'/.exec(sourceEntry?.[2] ?? '')?.[1] ?? id,
    variants, animated: id !== 'badge', desktop: classicBoard.some(x => x.id === id),
    source: sourceEntry ? at(formsRegistry, sourceEntry[0]) : at(bridge, id === 'badge' ? '  badge: BADGE_VARIANTS' : 'export const BRIDGED ='),
    variantSource: at('packages/render/src/classic/art.ts', 'export const variantsOf ='),
    needsContext: id === 'profile' ? 'projects and person' : id === 'blame' ? 'blame result' : null,
  };
});

const themeSource = read('packages/render/src/classic/themes.ts');
const themeFamilies = new Map([...themeSource.matchAll(/id: '([^']+)'[^\n]*family: '([^']+)'/g)].map(m => [m[1], m[2]]));
themeFamilies.set('minimal-dark', 'minimal'); // source inherits minimalLight
const styleDefinitions = new Map();
for (const name of ['base', 'east', 'west', 'story', 'pop', 'cyber']) {
  const file = `packages/render/src/styles/${name}.ts`;
  const source = read(file);
  const starts = [...source.matchAll(/^export const (\w+)(?:\s*:\s*Style)? = defineStyle\(\{/gm)];
  for (const [i, start] of starts.entries()) {
    const block = source.slice(start.index, starts[i + 1]?.index ?? source.length);
    const id = /id: '([^']+)'/.exec(block)[1];
    const name = /name: '([^']+)'/.exec(block)[1];
    const preferText = /prefer: (\{[^}]*\})/.exec(block)?.[1];
    const prefer = preferText ? vm.runInNewContext(`(${preferText})`, {}, {timeout:1000}) : {};
    styleDefinitions.set(start[1], { id, name, renderer: 'svg', prefer, source: at(file, `id: '${id}'`) });
  }
}
const styleOrder = value(stylesIndex, 'STYLES', Object.fromEntries(styleDefinitions));
const styles = [
  ...classicOrder.map(id => ({ id, name: classicNames[id].zh, renderer: 'classic', family: themeFamilies.get(id), source: at(catalog, id.includes('-') ? `'${id}':` : `  ${id}:`) })),
  ...styleOrder.filter(s => !skipped.includes(s.id)),
];
if (styles.length !== 26 || !styles.every(s => s?.id)) throw new Error(`Expected 26 baseline desktop styles; found ${styles.length}. Review changes explicitly.`);
const allStyles = [...styles.map(s=>({...s, desktop:true})), ...styleOrder.filter(s=>skipped.includes(s.id)).map(s=>({...s, desktop:false}))];

const commands = [...read(main).matchAll(/\{ id: '([^']+)', key: '([^']+)', label: \(\) => L\('([^']+)', '([^']+)'\)/g)].map(m => ({id:m[1], key:`CmdOrCtrl+${m[2]}`, name:m[3], en:m[4], source:at(main,m[0])}));
const native = 'apps/desktop/src-tauri/src/main.rs';
const nativeCommands = [native,'apps/desktop/src-tauri/src/preview.rs'].flatMap(file=>[...read(file).matchAll(/#\[tauri::command\]\n(?:pub )?(?:async )?fn (\w+)\(/g)].map(m => ({id:m[1], source:at(file,m[0])})));
const settingsKeys = uniq([...read(main).matchAll(/store\.(?:get|set)\('([^']+)'/g)].map(m => m[1])).map(id => ({key:`creditia.${id}${id.endsWith('.') ? '{projectId}' : ''}`, source:at(main, `'${id}'`)}));
const errors = Object.keys(value(main, 'ERRORS')).map(id => ({id, source:at(main, `  ${id}:`)}));
const features = JSON.parse(fs.readFileSync(path.join(here,'features.json'), 'utf8')).map(({anchor,...feature}) => ({...feature, baseline:feature.baseline ?? 'source_recorded', migration:'pending', ...(anchor ? {source:at(main, anchor)} : {})}));
const formats = value(exportFile, 'FORMATS').map(f => ({id:f.id, name:f.name, desktop:f.id !== 'embed', dragDock:!['gif','md','embed'].includes(f.id), condition:f.id === 'gif' ? 'animated artwork only' : f.id === 'md' ? 'clipboard with file fallback' : f.id === 'embed' ? 'shared declaration; hidden in desktop/CLI list' : null, source:at(exportFile,`{ id: '${f.id}'`)}));
const sizes = value(exportFile,'SIZES').map(s => ({...s,desktop:s.id !== 'a4', source:at(exportFile,`{ id: '${s.id}'`)}));
const fontFamilies = uniq([...read('packages/render/src/fonts.ts').matchAll(/\.\.\.F\('([^']+)'/g)].map(m => m[1]));
const classicFontFamilies = uniq([...read('packages/render/src/classic/fonts.ts').matchAll(/\.\.\.F\('([^']+)'/g)].map(m => m[1]));
const logoFile = 'apps/desktop/src/logo.ts';
const logos = [...read(logoFile).matchAll(/^  (?:'([^']+)'|(\w+)): \{ svg:/gm)].map(m => ({id:m[1]??m[2],source:at(logoFile,m[0])}));
const fixtures = [
  ['packages/render/fixtures/catnap.json','synthetic_render_sample','Rendering sample only; not a real project.'],
  ['crates/core/tests/fixtures/real/multi-tool.json','sanitized_observational_replay','Real bounded attribution replay; decoded engine input, not a complete visual credits archive.'],
  ['crates/core/tests/fixtures/real/multi-tool.expected.json','observational_expected','Regression baseline; not independently labeled accuracy ground truth.'],
].map(([file,kind,limits]) => ({path:file,kind,limits,sha256:createHash('sha256').update(read(file)).digest('hex')}));

const rows = [];
for (const style of allStyles) {
  const pieces = style.renderer === 'classic' ? classicPieces : v2Pieces;
  for (const piece of pieces) {
    const variants = piece.variants.length ? piece.variants : ['default'];
    for (const variant of variants) {
      const defaultVariant = style.renderer === 'classic'
        ? piece.id === 'app' ? 'themed' : defaults[piece.id]?.[style.family] ?? (piece.id === 'roll' ? 'classic' : piece.id === 'receipt' ? 'slip' : extras[piece.id] ? 'original' : variants[0])
        : style.prefer[piece.id] ?? variants[0];
      rows.push({style:style.id,renderer:style.renderer,piece:piece.id,variant,defaultVariant:variant === defaultVariant,desktop:style.desktop && piece.desktop,animated:piece.animated});
    }
  }
}
const localKeys = [
  ['Escape','close topmost popup/dialog/export list','function closeTop()'],
  ['Enter / Space','open focused wall artwork',"artEl.addEventListener('keydown'"],
  ['ArrowLeft / ArrowRight','previous/next layout in focus','  focusKeys = (e) =>'],
  ['ArrowUp / ArrowDown','previous/next artwork in focus','  focusKeys = (e) =>'],
  ['Arrow keys','pan roam viewport','function wireRoam()'],
  ['Home / 0','return roam viewport home','function wireRoam()'],
  ['ArrowUp / ArrowDown / Enter','select/open searched project','  q.onkeydown ='],
  ['Shift + wheel','horizontal roam scroll','function wireRoam()'],
].map(([key,action,anchor])=>({key,action,source:at(main,anchor)}));
const cliFile='packages/cli/src/cli.ts';
read(cliFile);
for (const file of ['apps/desktop/src/live-mark.ts','apps/desktop/src/roam-preview.ts','apps/desktop/src/wall.ts','apps/desktop/src/engine.ts','apps/desktop/src/export-worker.ts','apps/desktop/src/fonts.ts','apps/desktop/src/style.css','packages/render/src/mark.ts','packages/render/src/classic/motion.ts','packages/render/src/text.ts','packages/schema/credits.schema.json','packages/schema/types.ts','crates/core/src/config.rs','crates/core/tests/fixtures/real/README.md','packages/render/README.md']) read(file);
const inventory = {
  schemaVersion:1,baselineCommit:baseline,baselineMeaning:'migration scope reference; current source locations and fingerprints are captured separately and are not asserted to be byte-identical to this commit',sourceLocationBasis:'working_tree_at_capture',baselineStatus:'source_recorded',visualBaseline:'not_captured',gpuiAcceptance:'pending',
  counts:{styles:styles.length,classicStyles:classicOrder.length,svgStyles:styles.length-classicOrder.length,desktopUniquePieces:uniq([...classicBoard,...v2Board].map(x=>x.id)).length,sharedUniquePieces:uniq([...classicPieces,...v2Pieces].map(x=>x.id)).length,stylePieceLayoutRows:rows.length,desktopStylePieceLayoutRows:rows.filter(x=>x.desktop).length,features:features.length,globalShortcuts:commands.length,nativeCommands:nativeCommands.length},
  styles,sharedSvgStyles:styleOrder.map(s=>({...s,desktop:!skipped.includes(s.id)})),excludedSvgStyleAliases:skipped,boards:{classic:classicBoard,svg:v2Board},pieces:{classic:classicPieces,svg:v2Pieces},variantNames:{classic:classicVariantNames,svg:variantNames},
  features,globalShortcuts:commands,localShortcuts:localKeys,nativeCommands,settingsKeys,errors,logos,formats,sizes,
  additionalOutputs:[
    {id:'json',entry:'creditia export --format json / summary / publish',source:at(cliFile,"async function exportCmd()")},
    {id:'markdown-document',entry:'creditia doc',source:at(cliFile,"case 'doc':")},
    {id:'release-notes',entry:'creditia release-notes',source:at(cliFile,"case 'release-notes':")},
    {id:'pr-comment',entry:'creditia pr-comment / --commits',source:at(cliFile,"case 'pr-comment':")},
    {id:'badge-json',entry:'creditia badge-json',source:at(cliFile,"case 'badge-json':")},
    {id:'terminal',entry:'creditia fetch',source:at(cliFile,"case 'fetch':")},
    {id:'html-board',entry:'creditia board',source:at(cliFile,"case 'board':")},
    {id:'site',entry:'creditia site',source:at(cliFile,"case 'site':")},
  ],
  fonts:{families:fontFamilies,classicFamilies:classicFontFamilies,fontFilesCaptured:false,licenseAudit:'pending'},fixtures,
  observedOutputDifferences:[
    {id:'html-language',state:'shared_policy_implemented_acceptance_pending',detail:'The old desktop HTML path defaulted to zh while CLI supplied the selected language. Desktop, protocol and CLI now call the shared export policy with language; cross-entrypoint output and browser validation remain acceptance gates.',sources:[at('apps/desktop/src/export-worker.ts','return exportArtwork('),at('packages/render/src/export-job.ts',"case 'html':"),at(cliFile,'const response = await renderRequest(')]},
    {id:'pdf-raster-scale',state:'shared_policy_implemented_acceptance_pending',detail:'The old desktop capped PDF raster scale at 4000/max dimension while CLI defaulted to 3. The shared export policy now owns this rule; exact output checks must still run.',sources:[at('packages/render/src/export-job.ts',"case 'pdf':"),at(cliFile,'const response = await renderRequest(')]},
  ],
  candidateSourceFingerprints:[...seen].map(([file,source])=>({path:file,sha256:createHash('sha256').update(source).digest('hex')})).sort((a,b)=>a.path.localeCompare(b.path)),
};
function link(source) {return `[${source.path}:${source.line}](../../${source.path}#L${source.line})`;}
let md=`# 基线目录\n\n迁移范围参考主线：\`${baseline}\`。以下链接和行号对应本分支捕获时的源码；源码指纹单独记录，不声称等同于该主线提交。目录已登记，GPUI 功能/视觉验收均待验证。\n\n`;
md+=`桌面 **${styles.length} 种风格、${inventory.counts.desktopUniquePieces} 种作品**；共享注册表合计 ${inventory.counts.sharedUniquePieces} 种作品。台账有 ${rows.length} 个风格/作品/版式组合，其中 ${inventory.counts.desktopStylePieceLayoutRows} 个属于当前桌面可见作品。组合是否有足够输入数据，须运行后判定。\n\n`;
md+='## 26 个桌面风格\n\n| ID | 名称 | 渲染器 | 默认署名榜 / 字幕 | 源码 |\n|---|---|---|---|---|\n';
for (const s of styles) md+=`| ${s.id} | ${s.name} | ${s.renderer} | ${s.renderer==='classic' ? `original / ${defaults.roll[s.family]??'classic'}` : `${s.prefer.tier??'rows'} / ${s.prefer.roll??'classic'}`} | ${link(s.source)} |\n`;
md+='\nSVG 注册表另有 `minimal/noir/glass/cinema/terminal`；桌面为避免重复，只展示相应经典主题。名称相同不能证明内部画法相同，不要删除共享注册表能力。台账含这些共享样式的 215 个额外组合，以 renderer 区分同名主题；desktop=false，不计入桌面验收数量。\n';
for (const [label,pieces] of [['经典主题',classicPieces],['SVG 风格',v2Pieces]]) {
  md+=`\n## ${label}作品及全部版式\n\n| 作品 | 名称 | 版式 | 桌面 | 可生成动画帧 | 源码 |\n|---|---|---|---|---|---|\n`;
  for(const p of pieces)md+=`| ${p.id} | ${p.name} | ${p.variants.join(', ')||'默认'} | ${p.desktop?'是':'共享模块额外能力'} | ${p.animated?'是':'否'} | ${link(p.source)} |\n`;
}
md+='\n`profile` 需要项目集合与个人上下文；`blame` 需要溯源结果；`hand` 未列入默认桌面展墙。台账仍保留这些注册能力。经典主题追加版式经 `classic/bridge.ts` 使用其自己的颜色、字体及框线；不等于直接套用 SVG 风格。\n';
md+='\n## 全局快捷键\n\n| 操作 | macOS / Windows、Linux | 源码 |\n|---|---|---|\n';
for(const c of commands) md+=`| ${c.name} | ${c.key} | ${link(c.source)} |\n`;
md+='\n## 页面和操作\n\n| 功能 | 阶段 | 基线 | 检查项 |\n|---|---|---|---|\n';
for(const f of features)md+=`| ${f.name} | ${f.stage} | ${f.source?link(f.source):'本次新增要求'} | ${f.checks.join('；')} |\n`;
md+='\n## 输出边界\n\n| 格式 | 当前桌面 | 拖放坞 | 说明 |\n|---|---|---|---|\n';
for(const f of formats)md+=`| ${f.name} | ${f.desktop?'有':'隐藏'} | ${f.dragDock?'有':'无'} | ${f.condition??'共享导出器'} |\n`;
md+='\n- JSON、Markdown 文档、版本说明、PR/提交归因文字、动态徽章数据、终端名片、HTML 展墙及站点另由 CLI 提供。\n- 尺寸：native、og 1200×630、square 1080×1080、portrait 1080×1350、story 1080×1920、a4 2480×3508；A4 当前仅共享/CLI，桌面隐藏；CLI 支持自定义宽高。\n- SVG/HTML 文字可转路径；PDF 当前为带透明蒙版的栅格图像 PDF，不能宣称已实现可编辑矢量 PDF 或字体嵌入。\n- GIF 使用已有帧生成器；完整桌面动态播放与风格动作仍需迁移和补齐。开放的 [#116](https://github.com/WroughtMind/creditia/issues/116) 是待完成要求，不计为已交付基线。\n- `fixtures/catnap.json` 是虚构样例；真实回放样例是有限的归因输入，不是完整视觉档案。真实项目视觉样例和截图仍待采集。\n';
md+='\n基线中的 HTML 语言默认值与 PDF 栅格倍率差异，现已集中到 `packages/render/src/export-job.ts` 的共享规则。源码接入状态为 implemented；跨入口输出与浏览器验证仍为独立验收项，不能凭共用函数直接标为 verified。\n';

const header=['style','renderer','piece','variant','default_variant','desktop','animated','baseline_visual','gpui_functional','gpui_visual','gpui_motion','desktop_cli_web_outputs','evidence'];
const tsv=header.join('\t')+'\n'+rows.map(r=>[r.style,r.renderer,r.piece,r.variant,r.defaultVariant,r.desktop,r.animated,'pending','pending','pending',r.animated?'pending':'not_applicable','pending',''].join('\t')).join('\n')+'\n';
const outputs={'inventory.json':JSON.stringify(inventory,null,2)+'\n','CATALOG.md':md,'matrix.tsv':tsv};
if(process.argv.includes('--write')) {
  for(const [file,content] of Object.entries(outputs)) {
    // Preserve human acceptance columns for unchanged identities.
    if(file==='matrix.tsv' && fs.existsSync(path.join(here,file))) {
      const old=fs.readFileSync(path.join(here,file),'utf8').trimEnd().split('\n').slice(1).map(l=>l.split('\t'));
      const statuses=new Map(old.map(c=>[c.slice(0,4).join('\t'),c.slice(7)]));
      outputs[file]=header.join('\t')+'\n'+content.trimEnd().split('\n').slice(1).map(l=>{const c=l.split('\t');return [...c.slice(0,7),...(statuses.get(c.slice(0,4).join('\t'))??c.slice(7))].join('\t');}).join('\n')+'\n';
    }
    fs.writeFileSync(path.join(here,file),outputs[file]);
  }
  console.log(JSON.stringify(inventory.counts));
} else {
  for(const [file,content] of Object.entries(outputs)) {
    if(file==='matrix.tsv') {
      const saved=fs.readFileSync(path.join(here,file),'utf8').trimEnd().split('\n');
      const actual=content.trimEnd().split('\n');
      if(saved.length!==actual.length || saved.some((line,i)=>line.split('\t').slice(0,7).join('\t')!==actual[i].split('\t').slice(0,7).join('\t'))) throw new Error('matrix.tsv catalog drift; review and run --write');
    } else if(file==='inventory.json') {
      // Body changes during migration are allowed. Check catalog semantics, not
      // snapshot hashes or line shifts; at() above still verifies every anchor.
      const semantic=(_key,value)=>['candidateSourceFingerprints','source','variantSource'].includes(_key)?undefined:value;
      const saved=JSON.parse(fs.readFileSync(path.join(here,file),'utf8'));
      if(JSON.stringify(saved,semantic)!==JSON.stringify(inventory,semantic)) throw new Error(`${file} catalog drift; review and run --write`);
    }
  }
  console.log(`Inventory verified: ${styles.length} styles; ${rows.length} style/piece/layout combinations. Visual and GPUI acceptance remain separate.`);
}
