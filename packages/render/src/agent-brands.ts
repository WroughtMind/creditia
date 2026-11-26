import catalogue from '../../schema/agents.json' with { type: 'json' };

// Marks and primary colors from the cited Lobe Icons source snapshots.
// Black/white marks use contrast-aware ink in both renderer families.
export const AGENT_BRANDS: Record<string, { logo: string; color: string; onDark?: string }> = {
  "command-code": {"logo":"command-code","color":"#252525","onDark":"#E6E6E3"},
  "junie": {"logo":"junie","color":"#47E054"},
  "codeflicker": {"logo":"codeflicker","color":"#32EDDA"},
  "lovable": {"logo":"lovable","color":"#252525","onDark":"#E6E6E3"},
  "v0": {"logo":"v0","color":"#252525","onDark":"#E6E6E3"},
  "openclaw": {
    "logo": "openclaw",
    "color": "#ff4d4d"
  },
  "pi": {
    "logo": "pi",
    "color": "#252525",
    "onDark": "#E6E6E3"
  },
  "hermes-agent": {
    "logo": "hermes-agent",
    "color": "#252525",
    "onDark": "#E6E6E3"
  },
  "kiro": {
    "logo": "kiro",
    "color": "#9046ff"
  },
  "qoder": {
    "logo": "qoder",
    "color": "#252525",
    "onDark": "#E6E6E3"
  },
  "codebuddy": {
    "logo": "codebuddy",
    "color": "#6c4dff"
  },
  "mistral-vibe": {
    "logo": "mistral",
    "color": "#FA520F"
  },
  "minimax-code": {
    "logo": "minimax",
    "color": "#F23F5D"
  },
  "openhands": {
    "logo": "openhands",
    "color": "#252525",
    "onDark": "#E6E6E3"
  },
  "replit": {
    "logo": "replit",
    "color": "#FD5402"
  },
  "zencoder": {
    "logo": "zencoder",
    "color": "#E65C2C"
  }
};

/** 工具 → logo 名 */
export const LOGO_OF: Record<string, string> = {
  'claude-code': 'claude', codex: 'openai', cursor: 'cursor', grok: 'grok', kimi: 'kimi', gemini: 'geminicli', qwen: 'qwen', cline: 'cline',
  copilot: 'githubcopilot', amp: 'amp', windsurf: 'windsurf', trae: 'trae', deepseek: 'deepseek', 'github-actions': 'github-actions',
  dependabot: 'dependabot', renovate: 'renovate', opencode: 'opencode', roo: 'roocode', kilo: 'kilocode', devin: 'devin', jules: 'googlejules', aider: 'aider',
  crush: 'crush', goose: 'goose', continue: 'continue', zed: 'zed', 'amazon-q': 'amazonq', antigravity: 'antigravity',
};
/** 各家熟悉的品牌色（只在愿意用品牌色的风格里用） */
export const BRAND: Record<string, string> = {
  'claude-code': '#D97757', codex: '#10A37F', cursor: '#3B82F6', grok: '#6B6B70', kimi: '#2F6BFF', gemini: '#8E75B2', qwen: '#615CED',
  opencode: '#E8A33D', cline: '#5B8DEF', roo: '#C2410C', kilo: '#E5484D', copilot: '#6E40C9', aider: '#14B014', amp: '#F34E3F',
  windsurf: '#0FBF8F', trae: '#FF4D4F', crush: '#6B50FF', goose: '#1A1A1A', continue: '#3B3B3B', zed: '#084CCF', 'amazon-q': '#8C4FFF', antigravity: '#3279F9', deepseek: '#4D6BFE', 'github-actions': '#2088FF', devin: '#1F2937', jules: '#7C5CFF',
};

for (const [id, brand] of Object.entries(AGENT_BRANDS)) { LOGO_OF[id] = brand.logo; BRAND[id] = brand.color; }
LOGO_OF['trae-ide'] = LOGO_OF.trae; LOGO_OF['antigravity-ide'] = LOGO_OF.antigravity;
BRAND['trae-ide'] = BRAND.trae; BRAND['antigravity-ide'] = BRAND.antigravity;
for (const agent of catalogue.agents) {
  if (LOGO_OF[agent.harness]) LOGO_OF[agent.id] ??= LOGO_OF[agent.harness];
  if (BRAND[agent.harness]) BRAND[agent.id] ??= BRAND[agent.harness];
}
