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
