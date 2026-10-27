# Hero agent icon sources

Retrieved 2026-10-24–25. The Hero strip contains 77 brands: 40 from the shared renderer catalogue and 37 assets here. This continuation adds official inference.sh and MCPJam marks. Editor/CLI/region aliases share a mark; display is not a claim of complete integration.

Original source files are kept intact. The website builder uses `site/hero-icons.ts` and `site/hero-icons.py` (Pillow, already installed by Pages). Only the website display is made monochrome: vector foreground fills/strokes follow currentColor; embedded SVG styles are removed to prevent page-wide CSS leaks, and clip/mask IDs are scoped. Augment receives its original 512 × 512 coordinate box. Jazz, Reasonix and Eve omit their application tiles, Tinycloud keeps its ink outlines/eyes/smile, and Tabnine displays its unfilled symbol without wordmark letters. Raster marks use alpha or luminance masks and are painted with the same foreground: light ink for AiderDesk/fx/ZCode, dark ink for iFlow/Terramind, alpha silhouettes for Code Studio/Neovate/Moxby/CodeArts. Transparency and relative tonal detail remain; no brand RGB colors reach the Hero, including hover and dark mode. Kode selects the white product silhouette from the official README banner, with its green K retained as negative space; a 96px alpha mask is generated at build time so the browser does not load the full banner. Lingma uses its official white sidebar shape as an alpha mask. Rovo Dev retains the official outline strokes. Other renderer brand colors are unchanged.

Names and marks belong to their respective owners. Source licensing and trademark conditions still apply. ACP registry assets are the monochrome icons supplied in the official agent registry, pinned to its commit below.

| Asset | Source | Display / origin |
|---|---|---|
| `warp.svg` | [Warp](https://github.com/simple-icons/simple-icons/blob/90bd1f3e7b77851fd0c56474f6fa43b816babdab/icons/warp.svg) | Official asset; monochrome foreground at build time |
| `augment.svg` | [Augment](https://www.augmentcode.com/favicon.svg) | Official asset; monochrome foreground at build time |
| `aider-desk.png` | [AiderDesk](https://github.com/hotovo/aider-desk/blob/1958b399a2133d6a735a782a2a9676a7de568150/resources/icon.png) | Official asset; monochrome foreground at build time |
| `dexto.svg` | [Dexto](https://github.com/truffle-ai/dexto/blob/dbb401dabbf1b41a0ec6f7b39e1fb3266bffd701/docs/static/img/dexto/dexto_logo_icon.svg) | Official asset; monochrome foreground at build time |
| `forgecode.svg` | [ForgeCode](https://forgecode.dev/shared/brand/favicon.svg) | Official asset; monochrome foreground at build time |
| `jazz.svg` | [Jazz](https://github.com/lvndry/jazz/blob/579c2f6ef06005a6c1578ba7769afbf67b7547b3/.github/assets/jazz-icon.svg) | Official asset; monochrome foreground at build time |
| `pochi.svg` | [Pochi](https://github.com/TabbyML/pochi/blob/81ca233451a223e7897676a9e28ed4c9e20d92e4/packages/vscode/assets/icons/pochi-logo-dark.svg) | Official asset; monochrome foreground at build time |
| `astrbot.svg` | [AstrBot](https://github.com/AstrBotDevs/AstrBot/blob/0cd42bd4dd216d8810da4e3f5fb2e381d3c6817e/docs/public/favicon.svg) | Official asset; monochrome foreground at build time |
| `reasonix.svg` | [Reasonix](https://github.com/esengine/DeepSeek-Reasonix/blob/edfc6b12b6ea68e883eebbc057c061f65efba05c/desktop/electron/assets/icon.svg) | Official asset; monochrome foreground at build time |
| `posit-assistant.svg` | [Posit Assistant](https://assistant.posit.co/favicon.svg) | Official asset; monochrome foreground at build time |
| `tinycloud.svg` | [Tinycloud](https://www.tinycloud.sh/icon.svg) | Official asset; monochrome foreground at build time |
| `codestudio.ico` | [Code Studio](https://downloads.sfcodestudio.com/v2.0.5/mac-silicon/syncfusioncodestudio.dmg) | Official asset; monochrome foreground at build time |
| `sarvam-code.svg` | [Sarvam Code](https://marketplace.visualstudio.com/items?itemName=sarvam.sarvam-code) | Official asset; monochrome foreground at build time |
| `autohand.svg` | [Official source](https://raw.githubusercontent.com/agentclientprotocol/registry/fbcd742f1820233689e66a0a18785c6cd2e6dbb7/autohand/icon.svg) | Original official asset; theme foreground |
| `bob.svg` | [Official source](https://raw.githubusercontent.com/agentclientprotocol/registry/fbcd742f1820233689e66a0a18785c6cd2e6dbb7/bob/icon.svg) | Original official asset; theme foreground |
| `cortex-code.svg` | [Official source](https://raw.githubusercontent.com/agentclientprotocol/registry/fbcd742f1820233689e66a0a18785c6cd2e6dbb7/cortex-code/icon.svg) | Original official asset; theme foreground |
| `kimchi.svg` | [Official source](https://raw.githubusercontent.com/agentclientprotocol/registry/fbcd742f1820233689e66a0a18785c6cd2e6dbb7/kimchi/icon.svg) | Original official asset; theme foreground |
| `droid-mono.svg` | [Official source](https://raw.githubusercontent.com/agentclientprotocol/registry/fbcd742f1820233689e66a0a18785c6cd2e6dbb7/factory-droid/icon.svg) | Official ACP registry monochrome mark |
| `deepagents-mono.svg` | [Official source](https://raw.githubusercontent.com/agentclientprotocol/registry/fbcd742f1820233689e66a0a18785c6cd2e6dbb7/deepagents/icon.svg) | Official ACP registry symbol; replaces cropped wordmark |
| `adal.svg` | [Official source](https://adalagent.ai/adal-icon-pink.svg) | Original official asset; theme foreground |
| `mux.svg` | [Official source](https://raw.githubusercontent.com/coder/mux/9921714dd9da9b2abd54ec1cf474922f15cf4f0d/docs/favicon.svg) | Original official asset; theme foreground |
| `codearts-agent.svg` | [Official source](https://res-static.hc-cdn.cn/cloudbu-site/intl/zh-cn/codearts/logo.svg) | Original official asset; theme foreground |
| `iflow.png` | [Official source](https://img.alicdn.com/imgextra/i4/O1CN01yBfg3x1iNi4YggwIt_!!6000000004401-2-tps-72-72.png) | Original official asset; theme foreground |
| `fx.png` | [Official source](https://fx.sh/favicon.png) | Original official asset; theme foreground |
| `neovate.png` | [Official source](https://mdn.alipayobjects.com/huamei_9rin5s/afts/img/0uIJQaelzccAAAAAQCAAAAgADiB8AQFr/original) | Original official asset; theme foreground |
| `tabnine.svg` | [Official source](https://raw.githubusercontent.com/codota/tabnine-vscode/d6b83fea2dc5e915579a5e9b452719c59fde08e1/tabnine-logo-light.svg) | Official wordmark: symbol-only viewBox 0 0 25 25; preserve unfilled stroke geometry |
| `terramind.webp` | [Official source](https://terramind.com/images/logo-small.webp) | Original official asset; theme foreground |
| `zcode.png` | [Official source](https://zcode.z.ai/favicon-192x192.png?v=20260707-transparent) | Original official asset; theme foreground |
| `zenflow.svg` | [Official source](https://download.zencoder.ai/zenflowapp/latest/linux-x64/Zenflow.deb) | Zenflow 2.5.3 package: resources/frontend/zenflow-logo-small.svg |
| `firebender.svg` | [Official source](https://plugins.jetbrains.com/files/25224/1109733/Firebender-1.0.12.zip) | Firebender 1.0.12 JAR: META-INF/pluginIcon.svg |
| `moxby.png` | [Official source](https://moxby.com/install) | Official Chrome 3.2.0 CRX: icons/128.png; package hash in SUPPORT_AGENT8.md |
| `kode-header.png` | [Official source](https://github.com/shareAI-lab/Kode-CLI/blob/400bbf0449707846465895ef64cfbbd18bea4763/README.md) | Official README image [original](https://github.com/user-attachments/assets/c1751e92-94dc-4e4a-9558-8cd2d058c1a1); crop viewBox 40 50 94 100 from the 1982 × 958 banner; white-alpha threshold, painted with currentColor |
| `lingma.svg` | [Official source](https://open-vsx.org/api/Alibaba-Cloud/tongyi-lingma/2.6.10/file/Alibaba-Cloud.tongyi-lingma-2.6.10.vsix) | Official 2.6.10 package: extension/media/logo-sidebar.svg; original white silhouette as alpha mask |
| `rovodev.svg` | [Official source](https://github.com/atlassian/atlascode/blob/7ef48ac216e6ecb8da95ef5af92a94e5fea23f2d/resources/rovodev-sidebar-icon.svg) | Official Atlascode sidebar mark; outline painted with currentColor |
| `eve.svg` | [Official source](https://github.com/vercel/eve/blob/0a9f8579c822e6d056b3a7e32e2e09425e619f23/apps/docs/registry/channel/web/app/icon.svg) | Official Eve channel icon; omit the black tile and paint the original white geometry with currentColor |
| `inference.svg` | [Official site asset](https://cloud.inference.sh/web/assets/logo.svg), referenced by [inference.sh](https://inference.sh/) on 2026-10-25 | Preserve the three original paths, remove unused gradient definitions and paint with currentColor |
| `mcpjam.svg` | [Official Inspector asset](https://github.com/MCPJam/inspector/blob/c6c376ef07d5edf68d5f26775952def18b98cebe/mcpjam-inspector/client/public/mcp_jam.svg) | Remove the dark background tile; use original MCP / JAM letter geometry in viewBox 170 270 740 550, painted with currentColor |

## Asset hashes

SHA256 of unmodified source files:

| Asset | SHA256 |
|---|---|
| `inference.svg` | `6b2826cf7b4544b462bcebc24179800a436e66e5cbf96f71a8a6a4c58afac457` |
| `mcpjam.svg` | `99994144492cbc3a6a7cc35f8df992083e77f321f881f49accb9ca8fd5bb5a07` |
| `adal.svg` | `ac65bf43418a5c6f83e4f3a006bad46fc903fbbfbb961f11c8803b8f6ebec72d` |
| `aider-desk.png` | `5154bb7bfefbe1b93fe94fbc44de529ee59b5b7ceb07dc60b9d748c7857efbc1` |
| `astrbot.svg` | `5f17f2a86bbee1f6e0751c138f4b4bf7eed65ecef93a4bfecdf14982224e26cb` |
| `augment.svg` | `5151bb83fc6ee1f848f183923f25ea92fc2125ec0e7495e2a55b47dc4fd10e81` |
| `autohand.svg` | `6324e552f6c85025642ca9aabf4adddb4bba318a6f9422c7053fd4c610914120` |
| `bob.svg` | `c8c51486eda01dc5f31b32d4542cc469af02f86ca7558939b1618965fd515e56` |
| `codearts-agent.svg` | `d481170aa72810c9408302f6a242b7b724192b79a1e9f20afab541c596cc8728` |
| `codestudio.ico` | `4bea319b4837726bfb2074460f0a0f25f6a5902ac5b77676f2625e9b254cb37a` |
| `cortex-code.svg` | `70128bcb28473e73b53f93f9fea286004633acca8d378b9d97846463bdbefee1` |
| `deepagents-mono.svg` | `bb0fed2e9587992e5814c64df443109dd82520559f3527b1a3270477083712e6` |
| `dexto.svg` | `10f739e17d886db43b0b0967f91cc03fe97b7345a3617159fe96475f08f13817` |
| `droid-mono.svg` | `931960821fbafd095221a922ca017676d41fbc67f61b708d325413ad6e0bbec7` |
| `firebender.svg` | `83bb4d1b48ce59a0f4ad966077026e8bd5a8598d62042b87e910b4a5cfb4641f` |
| `forgecode.svg` | `e20dddf52248b06864d041886768c46466e8b54c3c2e4a4a73b660f4da8e8a1b` |
| `fx.png` | `b2367a695df27808db15c36c7eef8dde08f70b6d143cb34af74f7a75471e5f05` |
| `iflow.png` | `990d1032ea7e66f8550581e4bb0980a90dcaf996a1de75d4272ab14220412f36` |
| `jazz.svg` | `a111a33924ca630dd6dc8534d536a8dd7e52010aeb21533362aa2c1e31f2481e` |
| `kimchi.svg` | `d246ec242fb714ae57c66ce7cfd4d24a05ec092c50b7320c340cc10bf79c7aa4` |
| `moxby.png` | `315660b8acc3f455962e88978255f4cfeaf88d441c51262167bb6ed46f6e58a5` |
| `mux.svg` | `94e59c077967ada1abe8b516597da8f2cfd7d34d07110c3b49c1cdf142c09c4c` |
| `neovate.png` | `c19dbc3795a8c7ff163722afab03edddfd2ff24797e75b82ae07faa579b4968a` |
| `pochi.svg` | `f3d7af0a9bbc50412ae455ed4bfa2d9857119a904215561919ddaa09a69ff388` |
| `posit-assistant.svg` | `aede0d88d089cabe2594bc7a56e417bb1b3cdf40fa438af4898be4883408c5dc` |
| `reasonix.svg` | `44dcfc66e1c4c3d2ebf8d6e99460d7002413885d85ce8acc86f2e500cc75d242` |
| `sarvam-code.svg` | `3c83135c3bc95e36a8e2a1255292fd11315ff63a62382bc9f3ef5eae502ca7c1` |
| `tabnine.svg` | `a308fe68db87e11c2b8e5831ad1f64ca5cf9c3a73aa6b3c0001d028baf99a9c0` |
| `terramind.webp` | `a516bc9d5d082f7da96109dd50ab2202dc830d24508ddbe6e32c88d7a9f47bec` |
| `tinycloud.svg` | `ba5f83cd1633e40d72dbf99b409067160fa68742cb73b838526857e8c54a1486` |
| `warp.svg` | `dffc749e6bacd34f61e2349874bd3cef4971a20b04179db595cadb4bd4e188a3` |
| `zcode.png` | `089bdfa74910eaed241eac46bf5d02bb5718f15d8d6b247a1ee61b3372c9c82c` |
| `zenflow.svg` | `204483600d6318d8a07c105e821c978b0d96e82d002dc25cb0e37dd30416b69c` |
| `kode-header.png` | `8df9fa715ed49d7de3395a8e03319e4911a9bb7afb2edbe463254bf9ce3f143f` |
| `lingma.svg` | `463fed71f95ddcc6f6bd4629d9c93452a8559872dacd7e1e5faba5931292c0fa` |
| `rovodev.svg` | `f5b52a8254042d7a739b3747d69c7116aea6e51e67a0cdcf268478fa45f9f36a` |
| `eve.svg` | `a602a4540db4f1d2f9579f479f4c0c376ac5de960d6680d4ef7ab6632b6bf03f` |
