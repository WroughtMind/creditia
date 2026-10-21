# Logo 来源

全部为单色 SVG，仅用于识别对应工具，不表示任何合作或背书。各家使用规范需在发布前逐家核对。
改动后运行 `npm run logos`（packages/render）重新生成 `src/logos.ts`。

| 文件 | 来源 |
|---|---|
| openai.svg、cursor.svg、claude.svg、anthropic.svg、deepseek.svg、x.svg、kimi.svg | simple-icons（CC0） |
| googlegemini.svg、github-actions.svg、githubcopilot.svg、qwen.svg、cline.svg、amp.svg、windsurf.svg、trae.svg、openrouter.svg、dependabot.svg、renovate.svg | simple-icons 16.34（CC0） |

| opencode.svg、roocode.svg、kilocode.svg、devin.svg、geminicli.svg、grok.svg | @lobehub/icons-static-svg 1.95.1（MIT） |
| googlejules.svg | simple-icons 16.35（CC0） |
| aider.svg | Aider 仓库 aider/website/assets/icons/safari-pinned-tab.svg（Apache-2.0），把 potrace 的翻转变换算进了坐标 |

lobehub 的图标和 aider.svg 靠 evenodd 挖空，生成时会把 `rule: 'evenodd'` 带进 logos.ts。
还没有标志的工具画首字母。

creditia 自己的标志在 `../creditia/logo.svg`：由片尾字幕的横行组成的 C，开口处一颗四角星。彩色版 `../creditia/logo-color.svg` 的六道横纹是工具颜色（Claude Code、Codex、Cursor、Kimi、Grok、未确认），和宣传片结尾一样；App、图标、官网、README 用彩色版。
