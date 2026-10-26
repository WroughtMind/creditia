# Logo 来源

全部为单色 SVG，仅用于识别对应工具，不表示任何合作或背书。各家使用规范需在发布前逐家核对。
改动后运行 `npm run logos`（packages/render）重新生成 `src/logos.ts`。

| 文件 | 来源 |
|---|---|
| openai.svg、cursor.svg、claude.svg、anthropic.svg、deepseek.svg、x.svg、kimi.svg | simple-icons（CC0） |
| googlegemini.svg、github-actions.svg、githubcopilot.svg、qwen.svg、cline.svg、amp.svg、windsurf.svg、trae.svg、openrouter.svg、dependabot.svg、renovate.svg | simple-icons 16.34（CC0） |

| opencode.svg、roocode.svg、kilocode.svg、devin.svg、geminicli.svg、grok.svg | @lobehub/icons-static-svg 1.95.1（MIT） |
| googlejules.svg | simple-icons 16.35（CC0） |
| goose.svg、antigravity.svg | @lobehub/icons-static-svg 1.95.1（MIT） |
| zed.svg | simple-icons 16.34（CC0，zedindustries） |
| continue.svg | Continue 仓库 extensions/intellij/src/main/resources/icons/continue.svg（Apache-2.0） |
| amazonq.svg | aws-toolkit-vscode 仓库 packages/core/resources/icons/aws/amazonq/q-squid-ink.svg（Apache-2.0） |
| crush.svg | Crush 仓库 internal/oauth/callback/heartbit.svg：心形轮廓，眼睛和嘴挖空成单色，去掉腮红 |
| aider.svg | Aider 仓库 aider/website/assets/icons/safari-pinned-tab.svg（Apache-2.0），把 potrace 的翻转变换算进了坐标 |

lobehub 的图标和 aider.svg 靠 evenodd 挖空，生成时会把 `rule: 'evenodd'` 带进 logos.ts。

creditia 自己的标志在 `../creditia/`：`logo-star.svg` 是五瓣星号 *（默认，App 图标、官网、README 用它），每瓣一个工具色（Claude Code、Codex、Cursor、Kimi、Grok）；排版里 * 就是「出处在这儿」。`logo-star-mono.svg` 是它的黑白版。两个都由 `npm run mark`（`src/mark.ts`）生成，作品落款处每个项目自己的 *（瓣长是份额）也出自那里。早先的几个也留着：`logo-orb.svg` 彩色横纹的圆、星从右上亮出来；`logo-ring.svg` 六道胶囊围成一圈、星在正中；`logo-color.svg` / `logo.svg` 是最早的 C 字标志（彩色 / 黑白）。这些都能在 App 设置里换。

## Agent catalogue additions (2026-10-23)

- openclaw: [Lobe Icons OpenClaw](https://github.com/lobehub/lobe-icons/blob/master/src/OpenClaw/components/Mono.tsx), source blob `924e2920337428114909e2a8603512f23a2daac4`; primary color from `src/OpenClaw/style.ts` (2026-10-23).
- pi: [Lobe Icons Pi](https://github.com/lobehub/lobe-icons/blob/master/src/Pi/components/Mono.tsx), source blob `0fbb3b8bd5d588aba8ba82c16c07f50b09fd4429`; primary color from `src/Pi/style.ts` (2026-10-23).
- hermes-agent: [Lobe Icons HermesAgent](https://github.com/lobehub/lobe-icons/blob/master/src/HermesAgent/components/Mono.tsx), source blob `42f5154114f6acbd0a4796293847ace084735458`; primary color from `src/HermesAgent/style.ts` (2026-10-23).
- kiro: [Lobe Icons Kiro](https://github.com/lobehub/lobe-icons/blob/master/src/Kiro/components/Mono.tsx), source blob `fc10dc2f1c2c18a80a61e51ad96cb9ffd50b0f95`; primary color from `src/Kiro/style.ts` (2026-10-23).
- qoder: [Lobe Icons Qoder](https://github.com/lobehub/lobe-icons/blob/master/src/Qoder/components/Mono.tsx), source blob `4c6e88342fb5363194bdbf5f9692849cc69d00df`; primary color from `src/Qoder/style.ts` (2026-10-23).
- codebuddy: [Lobe Icons CodeBuddy](https://github.com/lobehub/lobe-icons/blob/master/src/CodeBuddy/components/Mono.tsx), source blob `fdfa3ecaa7a650fc5bda565baded130402c15ae5`; primary color from `src/CodeBuddy/style.ts` (2026-10-23).
- mistral: [Lobe Icons Mistral](https://github.com/lobehub/lobe-icons/blob/master/src/Mistral/components/Mono.tsx), source blob `d99b0e8dfdd063af88056d68193fde7a6594c74b`; primary color from `src/Mistral/style.ts` (2026-10-23).
- minimax: [Lobe Icons Minimax](https://github.com/lobehub/lobe-icons/blob/master/src/Minimax/components/Mono.tsx), source blob `e362af64ffff8509580bb80e170aa093b944b24a`; primary color from `src/Minimax/style.ts` (2026-10-23).
- openhands: [Lobe Icons OpenHands](https://github.com/lobehub/lobe-icons/blob/master/src/OpenHands/components/Mono.tsx), source blob `3193ed0654b0e9ee87a896520cb495ec391374df`; primary color from `src/OpenHands/style.ts` (2026-10-23).
- replit: [Lobe Icons Replit](https://github.com/lobehub/lobe-icons/blob/master/src/Replit/components/Mono.tsx), source blob `f50d44684d530681e3e93238047cbea45dc58ad0`; primary color from `src/Replit/style.ts` (2026-10-23).
- zencoder: [Lobe Icons Zencoder](https://github.com/lobehub/lobe-icons/blob/master/src/Zencoder/components/Mono.tsx), source blob `d2001c2c5de35df98d7a2f01a8080d3ec994bdec`; primary color from `src/Zencoder/style.ts` (2026-10-23).
# Additional agent marks, checked 2026-10-23

- `command-code.svg`: [Lobe Icons](https://github.com/lobehub/lobe-icons/blob/master/src/CommandCode/components/Mono.tsx), blob `e9b49ceccb7b393d5dccb1d445be9aa7db23c4c4`; [primary color](https://github.com/lobehub/lobe-icons/blob/master/src/CommandCode/style.ts), blob `a95fe244fe44d9bc3bfa2307227ab757345c1a5f`.
- `junie.svg`: [Lobe Icons](https://github.com/lobehub/lobe-icons/blob/master/src/Junie/components/Mono.tsx), blob `7ea8f005ee63d904ec6c83fe2a5fdc624b42ee4a`; [primary color](https://github.com/lobehub/lobe-icons/blob/master/src/Junie/style.ts), blob `55cfaa45ec646043a538941dd5cb340819f2390c`.
- `codeflicker.svg`: [Lobe Icons](https://github.com/lobehub/lobe-icons/blob/master/src/CodeFlicker/components/Mono.tsx), blob `fc60d5d868a7eba3eca8c0b04379171d7003f95a`; [primary color](https://github.com/lobehub/lobe-icons/blob/master/src/CodeFlicker/style.ts), blob `426457df14dcbd371c721207087d87ed476ac8f5`.
- `lovable.svg`: [Lobe Icons](https://github.com/lobehub/lobe-icons/blob/master/src/Lovable/components/Mono.tsx), blob `c501deca66107f80a5fbbdc4bb5abf7c839c94d9`; [primary color](https://github.com/lobehub/lobe-icons/blob/master/src/Lovable/style.ts), blob `0a4b01e5067f81629cf0bf58ab6871f079b73e2a`.
- `v0.svg`: [Lobe Icons](https://github.com/lobehub/lobe-icons/blob/master/src/V0/components/Mono.tsx), blob `e1890cb967eefc28cc0a697b1c6434e331afb15f`; [primary color](https://github.com/lobehub/lobe-icons/blob/master/src/V0/style.ts), blob `fbd1bd14a665030758281560a1c86a8f3d0eff31`.
