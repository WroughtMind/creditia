import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { renderRequest, nativeBackend } from '../src/native-protocol.ts';
import { createRenderSession } from '../src/protocol.ts';
import { buildModel } from '../src/classic/data.ts';
import { markSvg, sharePetals, MARK_GREY } from '../src/mark.ts';
import { agentOf } from '../src/data.ts';

const raw = JSON.parse(readFileSync(new URL('../fixtures/catnap.json', import.meta.url), 'utf8'));
const result = (response: any): any => {
  assert.equal(response.ok, true, JSON.stringify(response.error));
  return response.result;
};

test('project marks preserve the original grouped shares, palette and private-name setting', async () => {
  for (const group of ['auto', 'tool', 'model'] as const) for (const lang of ['zh', 'en'] as const) for (const named of [true, false]) {
    const model = buildModel(raw, { group, lang, named, director: false });
    const petals = sharePetals(model.agents.filter(agent => agent.kind === 'agent').map(agent => ({ tool: agent.key.split('/')[0], lines: agent.lines })));
    const actual = result(await renderRequest({ version: 1, id: 'mark', op: 'mark', raw, group, lang, named, sign: 'private-signature' }));
    assert.equal(actual.hasModels, raw.contributors.some((person: any) => !!agentOf(raw, person.id)?.model));
    assert.equal(actual.resolvedGroup, group === 'model' ? 'model' : 'tool');
    assert.equal(actual.project, model.project);
    assert.deepEqual(actual.rows, model.rows.filter(row => ['agent', 'unknown', 'other'].includes(row.kind)).map(({ name, pct, kind }) => ({ name, pct, kind })));
    assert.equal(actual.svg, markSvg({ petals, optical: true, size: 384 }));
    assert.equal(actual.rowSvg, markSvg({ petals, optical: true, size: 96 }));
    assert.deepEqual(actual.petals.map(({ svg, ...petal }: any) => petal), petals);
    actual.petals.forEach((petal: any, index: number) => {
      assert.equal(petal.svg, markSvg({ optical: true, size: 384, petals: petals.map((p, i) => i === index ? p : { ...p, opacity: 0 }) }));
      assert.equal((petal.svg.match(/<path /g) ?? []).length, 1);
    });
    assert.ok(!JSON.stringify(actual).includes('private-signature'));
    for (const person of raw.contributors.filter((person: any) => !person.harness && person.name)) {
      assert.ok(!JSON.stringify(actual).includes(person.name), 'status petals never contain private contributor names');
    }
  }
});

test('project marks retain tools after the fifth and accept an empty archive', async () => {
  const many = {
    contributors: Array.from({ length: 8 }, (_, i) => ({ id: `a:${i}`, kind: 'agent', harness: `tool-${i}` })),
    breakdown: Array.from({ length: 8 }, (_, i) => ({ contributor: `a:${i}`, linesAdded: i === 0 ? 10000 : 1 })),
  };
  const actual = result(await renderRequest({ version: 1, id: 'many', op: 'mark', raw: many }));
  assert.equal(actual.petals.length, 8);
  assert.equal(actual.hasModels, false);
  assert.equal((actual.svg.match(/<path /g) ?? []).length, 8);
  assert.doesNotMatch(actual.svg, /NaN|Infinity/);
  const empty = result(await renderRequest({ version: 1, id: 'empty', op: 'mark', raw: {} }));
  assert.equal(empty.petals.length, 5);
  assert.equal(empty.hasModels, false);
  assert.ok(empty.petals.every((petal: any) => petal.color === MARK_GREY && petal.opacity === 0.35));
  assert.equal((await renderRequest({ version: 1, id: 'bad', op: 'mark', raw: null })).ok, false);
});

test('project grouping visibility follows recorded models including legacy archives', async () => {
  for (const contributor of [
    { id: 'a:claude-code/claude-opus-4', kind: 'agent' },
    { id: 'a:1', kind: 'agent', harness: 'codex', model: 'gpt-5' },
  ]) {
    const actual = result(await renderRequest({ version: 1, id: 'models', op: 'mark', raw: { contributors: [contributor] } }));
    assert.equal(actual.hasModels, true);
  }
  const person = result(await renderRequest({ version: 1, id: 'person', op: 'mark', raw: { contributors: [{ id: 'h:1', kind: 'person', name: 'Example', model: 'annotation' }] } }));
  assert.equal(person.hasModels, false);
});

test('registered and direct project marks use identical archive and option snapshots', async () => {
  const session = createRenderSession(nativeBackend);
  result(await session.dispatch({ version: 1, id: 'register', op: 'archive', archiveId: 'project', raw }));
  const options = { version: 1, id: 'mark', op: 'mark', group: 'model', named: false, lang: 'en' };
  assert.deepEqual(result(await session.dispatch({ ...options, archiveId: 'project' })), result(await renderRequest({ ...options, raw })));
  const invalid = await session.dispatch({ ...options, archiveId: 'project', unexpected: true });
  assert.equal(invalid.ok, false);
});
