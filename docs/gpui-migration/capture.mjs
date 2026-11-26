#!/usr/bin/env node
// Compare the current shared artwork catalog with the recorded acceptance scope.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../..');
// Catalogs and JSON fixtures are source text: Git checkout line endings must
// not alter semantic inventory or text fingerprints. Render evidence separately
// records the exact input bytes used for each run.
const seen = new Map();
function read(file) {
  if (!seen.has(file)) seen.set(file, fs.readFileSync(path.join(root, file), 'utf8').replace(/\r\n/g, '\n'));
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
const boards = 'packages/render/src/boards.ts';
const registry = 'packages/render/src/registry.ts';
const classicCatalog = 'packages/render/src/classic/catalog.ts';
const formsRegistry = 'packages/render/src/classic/registry.ts';
const forms = 'packages/render/src/classic/forms.ts';
const bridge = 'packages/render/src/classic/bridge.ts';
const stylesIndex = 'packages/render/src/styles/index.ts';
const exportFile = 'packages/render/src/export.ts';
const classicBoard = value(boards, 'CLASSIC_BOARD');
const v2Board = value(boards, 'V2_BOARD');
const classicNames = value(classicCatalog, 'THEME_INFO');
const classicOrder = value(classicCatalog, 'THEME_ORDER');
const skipped = [...value('packages/render/src/protocol.ts', 'SKIP_STYLES')];

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
const classicVariantNames = { ...variantNames, ...value(classicCatalog, 'VARIANT_INFO') };
const pieceInfo = value(classicCatalog, 'PIECE_INFO');

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
    variants, animated: animatedClassic(id), desktop: classicBoard.some(x => x.id === id),
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
  ...classicOrder.map(id => ({ id, name: classicNames[id].zh, renderer: 'classic', family: themeFamilies.get(id), source: at(classicCatalog, id.includes('-') ? `'${id}':` : `  ${id}:`) })),
  ...styleOrder.filter(s => !skipped.includes(s.id)),
];
if (styles.length !== 26 || !styles.every(s => s?.id)) throw new Error(`Expected 26 desktop styles; found ${styles.length}. Review changes explicitly.`);
const allStyles = [...styles.map(s=>({...s, desktop:true})), ...styleOrder.filter(s=>skipped.includes(s.id)).map(s=>({...s, desktop:false}))];

const formats = value(exportFile, 'FORMATS').map(f => ({id:f.id, name:f.name, desktop:f.id !== 'embed', dragDock:!['gif','md','embed'].includes(f.id), condition:f.id === 'gif' ? 'animated artwork only' : f.id === 'md' ? 'clipboard with file fallback' : f.id === 'embed' ? 'shared declaration; hidden in desktop/CLI list' : null, source:at(exportFile,`{ id: '${f.id}'`)}));
const sizes = value(exportFile,'SIZES').map(s => ({...s,desktop:s.id !== 'a4', source:at(exportFile,`{ id: '${s.id}'`)}));
const fontFamilies = uniq([...read('packages/render/src/fonts.ts').matchAll(/\.\.\.F\('([^']+)'/g)].map(m => m[1]));
const classicFontFamilies = uniq([...read('packages/render/src/classic/fonts.ts').matchAll(/\.\.\.F\('([^']+)'/g)].map(m => m[1]));
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
const inventoryPath = path.join(here, 'inventory.json');
const inventory = JSON.parse(fs.readFileSync(inventoryPath, 'utf8'));
const counts = {
  styles: styles.length,
  classicStyles: classicOrder.length,
  svgStyles: styles.length - classicOrder.length,
  desktopUniquePieces: uniq([...classicBoard, ...v2Board].map(x => x.id)).length,
  sharedUniquePieces: uniq([...classicPieces, ...v2Pieces].map(x => x.id)).length,
  stylePieceLayoutRows: rows.length,
  desktopStylePieceLayoutRows: rows.filter(x => x.desktop).length,
};
const catalog = {
  styles, sharedSvgStyles: styleOrder.map(s => ({...s, desktop: !skipped.includes(s.id)})),
  excludedSvgStyleAliases: skipped, boards: {classic: classicBoard, svg: v2Board},
  pieces: {classic: classicPieces, svg: v2Pieces},
  variantNames: {classic: classicVariantNames, svg: variantNames}, formats, sizes,
  fonts: {...inventory.fonts, families: fontFamilies, classicFamilies: classicFontFamilies},
};
const header=['style','renderer','piece','variant','default_variant','desktop','animated','baseline_visual','gpui_functional','gpui_visual','gpui_motion','desktop_cli_web_outputs','evidence'];
const tsv=header.join('\t')+'\n'+rows.map(r=>[r.style,r.renderer,r.piece,r.variant,r.defaultVariant,r.desktop,r.animated,'pending','pending','pending',r.animated?'pending':'not_applicable','pending',''].join('\t')).join('\n')+'\n';
const matrixPath = path.join(here, 'matrix.tsv');
const matrix = text => text.trimEnd().split('\n').map(line => line.split('\t'));
const actual = matrix(tsv);
const saved = matrix(fs.readFileSync(matrixPath, 'utf8'));
if (process.argv.includes('--write')) {
  const statuses = new Map(saved.slice(1).map(columns => [columns.slice(0,4).join('\t'), columns.slice(7)]));
  const updated = actual.slice(1).map(columns => [
    ...columns.slice(0,7), ...(statuses.get(columns.slice(0,4).join('\t')) ?? columns.slice(7)),
  ].join('\t'));
  fs.writeFileSync(matrixPath, header.join('\t') + '\n' + updated.join('\n') + '\n');
  fs.writeFileSync(inventoryPath, JSON.stringify({...inventory, ...catalog, counts:{...inventory.counts, ...counts}}, null, 2) + '\n');
} else {
  if (saved.length !== actual.length || saved.some((columns, i) => columns.slice(0,7).join('\t') !== actual[i].slice(0,7).join('\t'))) {
    throw new Error('matrix.tsv catalog drift; review and run --write');
  }
  const semantic = (key, value) => ['source','variantSource'].includes(key) ? undefined : value;
  const recorded = Object.fromEntries(Object.keys(catalog).map(key => [key, inventory[key]]));
  if (Object.entries(counts).some(([key, value]) => inventory.counts[key] !== value) ||
      JSON.stringify(recorded, semantic) !== JSON.stringify(catalog, semantic)) {
    throw new Error('inventory.json catalog drift; review and run --write');
  }
}
console.log(`Inventory verified: ${styles.length} styles; ${rows.length} style/piece/layout combinations.`);
import { animatedClassic } from '../../packages/render/src/classic/art.ts';
