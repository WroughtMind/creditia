import { test } from 'node:test';
import assert from 'node:assert/strict';
import catalogue from '../../schema/agents.json' with { type: 'json' };
import { TOOL_NAMES } from '../src/tool-names.ts';
import { buildModel, art, STYLES } from '../src/index.ts';
import * as classic from '../src/classic/index.ts';
import { AGENT_BRANDS } from '../src/agent-brands.ts';
import { LOGO_OF } from '../src/style.ts';
import { LOGOS } from '../src/logos.ts';
import { BRANDS, logoSvg, colorOf } from '../src/classic/brand.ts';
import { mark } from '../src/classic/ui.ts';

test('every catalogue entry reaches both renderers under its canonical tool identity', async () => {
  for (const agent of catalogue.agents) {
    const key = agent.harness;
    const raw = { project: { name: 'Agent rendering fixture' }, contributors: [{ id: `a:${key}`, kind: 'agent', harness: key, roles: ['author'] }], breakdown: [{ contributor: `a:${key}`, linesAdded: 20, linesSurviving: 20 }], totals: { linesAdded: 20, linesSurviving: 20 } };
    const modern = buildModel(raw, { group: 'tool' });
    const old = classic.buildModel(raw, { director: false, lang: 'zh', group: 'tool' });
    assert.equal(modern.rows[0].name, TOOL_NAMES[key], agent.id);
    assert.equal(old.rows[0].name, TOOL_NAMES[key], agent.id);
    assert.equal(modern.rows[0].pct, 100, agent.id);
    assert.equal(old.rows[0].pct, 100, agent.id);
    const svg = art('badge', modern, STYLES[0])!.svg;
    const oldSvg = (await classic.art('badge', old, classic.themeById('minimal-light')!, { lang: 'zh' }))!.svg;
    for (const result of [svg, oldSvg]) assert.ok(result.startsWith('<svg') && !/NaN|undefined/.test(result), agent.id);
    // Credits must include authors even when no sourced logo is available.
    for (const variant of ['classic', 'opening', 'production', 'slate', 'crew']) {
      const output = classic.FORMS.roll.render(old, classic.themeById('cinema')!, { lang: 'zh', variant });
      assert.ok(JSON.stringify(output.tree).includes(TOOL_NAMES[key]), `${agent.id}/${variant} omitted its author`);
    }
    if (!LOGO_OF[key]) {
      const initials = TOOL_NAMES[key].split(/[\s-]+/).map((w) => w[0]).join('').slice(0, 2).toUpperCase();
      assert.ok(JSON.stringify(mark(key, classic.themeById('minimal-light')!, 24)).includes(initials), agent.id);
    }
  }
});

test('new sourced marks resolve identically in both renderers, including model grouping', () => {
  for (const [id, brand] of Object.entries(AGENT_BRANDS)) {
    assert.equal(LOGO_OF[id], BRANDS[id].logo, id);
    assert.ok(LOGOS[brand.logo]?.d.length > 0, id);
    assert.ok(logoSvg(`${id}/some-model`)?.includes(LOGOS[brand.logo].d), id);
    if (brand.onDark) assert.equal(colorOf(id, classic.themeById('cinema')!), brand.onDark, id);
  }
  for (const a of catalogue.agents) if (a.id !== 'antigravity') assert.equal(LOGO_OF[a.id], LOGO_OF[a.harness], a.id);
});
