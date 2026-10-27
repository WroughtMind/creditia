import { spawnSync } from 'node:child_process';
import { AGENT_BRANDS } from '../packages/render/src/agent-brands.ts';
import { LOGOS } from '../packages/render/src/logos.ts';
import { LOGO_OF } from '../packages/render/src/style.ts';
import { TOOL_NAMES } from '../packages/render/src/data.ts';
import { SITE_AGENT_ICONS } from './agent-icons.ts';

/** Aliases share one mark. Logo presence does not assert complete history coverage. */
export function heroIcons() {
  const shared = ['claude-code', 'codex', 'cursor', 'gemini', 'qwen', 'copilot', 'windsurf', 'trae', 'antigravity', 'opencode', 'cline', 'roo', 'kilo', 'kimi', 'grok', 'amp', 'zed', 'amazon-q', 'continue', 'goose', 'crush', 'aider', 'devin', 'jules', ...Object.keys(AGENT_BRANDS)];
  const result = spawnSync('python3', [`${import.meta.dirname}/hero-icons.py`], {
    input: JSON.stringify(SITE_AGENT_ICONS), encoding: 'utf8', maxBuffer: 4 * 1024 * 1024,
  });
  if (result.status !== 0) throw new Error(`Hero icon conversion failed: ${result.stderr || result.error}`);
  const extra: Record<string, string> = JSON.parse(result.stdout);
  return [...shared, ...Object.keys(SITE_AGENT_ICONS)].map((id) => {
    const mark = LOGOS[LOGO_OF[id] ?? ''];
    const svg = extra[id] ?? (mark && `<svg viewBox="${mark.vb}" aria-hidden="true"><path d="${mark.d}"${mark.rule ? ` fill-rule="${mark.rule}"` : ''}/></svg>`);
    if (!svg) throw new Error(`Missing Hero mark: ${id}`);
    return { id, name: TOOL_NAMES[id] ?? id, svg };
  });
}
