import fs from 'node:fs';
import { sha256 } from './common.mjs';

/** Derived fixtures are explicitly synthetic, even when derived from a real archive. */
export function fixtureCases(filename, kind = 'synthetic_render_sample') {
  const bytes = fs.readFileSync(filename), base = JSON.parse(bytes);
  if (!base || !Array.isArray(base.contributors) || !base.project) throw Error('Expected a complete credits archive; engine replay inputs are not visual archives');
  const make = (id, raw, label) => ({ id, raw, provenance: { kind: label, sha256: sha256(JSON.stringify(raw)), source_sha256: sha256(bytes) } });
  const long = structuredClone(base);
  long.project.name = '中文长项目名：贡献归因、作品排版与跨平台动画验证／Creditia 2026';
  for (const [i, person] of (long.contributors ?? []).entries()) if (person.kind === 'human') person.name = `贡献者${i}：司徒长风与赵钱孙李周吴郑王`;
  const empty = structuredClone(base);
  empty.project.name = '空档案 / Empty archive';
  for (const key of ['contributors', 'sessions', 'files', 'versions', 'units', 'coverage', 'directors', 'areas']) if (Array.isArray(empty[key])) empty[key] = [];
  for (const key of ['attribution', 'history', 'totals', 'stats']) if (empty[key] && typeof empty[key] === 'object') empty[key] = {};
  return [make('source', base, kind), make('long-zh', long, 'synthetic_stress_derived'), make('empty', empty, 'synthetic_empty_derived')];
}

export function contexts(raw) {
  const second = structuredClone(raw);
  second.project = { ...second.project, name: `${second.project.name} / second fixture`, repos: ['github.com/example/second-acceptance-fixture'] };
  return {
    projects: [raw, second], person: 'Acceptance fixture / 验收样例',
    blame: { file: 'src/acceptance-fixture.rs', line: 12, harness: 'codex', model: 'fixture-model',
      commit: '0000000', subject: 'Synthetic blame presentation fixture', evidence: 'declared', date: '2026-01-01' },
  };
}

export const preferences = [
  { id: 'zh-auto-named', lang: 'zh', group: 'auto', named: true },
  { id: 'en-tool-private', lang: 'en', group: 'tool', named: false },
  { id: 'zh-model-private', lang: 'zh', group: 'model', named: false },
  { id: 'en-model-named', lang: 'en', group: 'model', named: true },
];
