import catalogue from '../../schema/agents.json' with { type: 'json' };
// Stable tool identities shared by every display. Models have their own names.
export const TOOL_NAMES: Record<string, string> = {
  'claude-code': 'Claude Code', codex: 'Codex', cursor: 'Cursor', grok: 'Grok Build', kimi: 'Kimi Code CLI',
  gemini: 'Gemini CLI', qwen: 'Qwen Code', opencode: 'OpenCode', cline: 'Cline', roo: 'Roo Code',
  kilo: 'Kilo Code', copilot: 'GitHub Copilot', aider: 'Aider', devin: 'Devin', jules: 'Jules', amp: 'Amp',
  windsurf: 'Windsurf Cascade', trae: 'TRAE CLI', crush: 'Crush', goose: 'Goose', continue: 'Continue',
  zed: 'Zed', 'amazon-q': 'Amazon Q', antigravity: 'Antigravity CLI', zcode: 'ZCode', pi: 'Pi',
  omp: 'Oh My Pi', 'command-code': 'Command Code', muse: 'Muse Code', t3: 'T3 Code',
  'mistral-vibe': 'Mistral Vibe', 'minimax-code': 'MiniMax Code', droid: 'Factory Droid', auggie: 'Auggie CLI',
  qoder: 'Qoder', codebuddy: 'CodeBuddy Code', junie: 'Junie', kiro: 'Kiro CLI', iflow: 'iFlow CLI',
  neovate: 'Neovate Code', costrict: 'CoStrict', codeflicker: 'CodeFlicker', tabnine: 'Tabnine Agent', comate: 'Comate / Zulu',
  openhands: 'OpenHands', 'swe-agent': 'SWE-agent', 'mini-swe-agent': 'mini-SWE-agent',
  replit: 'Replit Agent', bolt: 'Bolt.new', lovable: 'Lovable', v0: 'v0', warp: 'Warp Agent',
  'trae-ide': 'TRAE', 'antigravity-ide': 'Antigravity', dsh: 'DeepSeek Harness',
  'cortex-code': 'Cortex Code', bob: 'IBM Bob', poolside: 'Poolside', autohand: 'Autohand Code',
  vtcode: 'VT Code', kimchi: 'Kimchi', nova: 'Nova', 'glm-agent': 'GLM Agent', dirac: 'Dirac',
  corust: 'Corust Agent', 'crow-cli': 'crow-cli', dimcode: 'DimCode', 'minion-code': 'Minion Code',
  sigit: 'siGit Code', stakpak: 'Stakpak', deepseek: 'DeepSeek',
  'github-actions': 'GitHub Actions', dependabot: 'Dependabot', renovate: 'Renovate',
};

// Public aliases come from the same catalogue used by the Rust CLI.
for (const a of catalogue.agents) {
  TOOL_NAMES[a.id] ??= a.name;
  TOOL_NAMES[a.harness] ??= a.name;
}
