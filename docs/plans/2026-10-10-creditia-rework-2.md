# creditia 第二轮重做：体裁、真电影片尾、App 使用逻辑

日期：2026-10-10
前置：`docs/plans/2026-10-08-creditia-plan.md`（第一轮）、`docs/plans/2026-10-08-creditia-forms.md`

## 1. 为什么重做

用户第二次验收的意见（原文）：

> bro你真的是fable吗??这app也太鸡儿丑了,而且片尾字幕还有很多样式能做啊...你真没看过电影吗???然后这很多样式也是都可以改进的.并且这个app的使用逻辑也很傻很傻.你还塞了一大堆毫无必要的文案

> not only problem of 片尾字幕哈...其他很多样式的也是,而且风格你没必要一定这么禁欲这么简约现代吧,我说的是app要现代没说这些风格

拆成四件事：

1. **出图风格太禁欲**。上一轮把「现代」理解错了——用户要的是 **App 现代**，出图风格反而应该像真实世界的片尾字幕、戏院海报、颁奖礼、老报纸那样各有装饰和戏剧感。
2. **片尾字幕只有五种，且不像真电影**。真电影的片尾有滚屏、卡司表、billing block、标题卡、卡串、终卡等成套语汇。
3. **其他形式也单一**，同样需要更多版式与家族化处理。
4. **App 使用逻辑傻、文案灌水**。主线应该只有一条：挑一张图拿走。

## 2. 硬约束

- **现有的全部保留，只增不删**（用户明确要求）：七个主题（minimal-light/minimal-dark/film/terminal/glass/editorial/mono）、片尾五种版式、小票四种版式、所有形式与分组，一个都不动；新的全部是追加。
- 对 `~/projects/魏碑` 只读，永远用 `--out` / `--config spike/out/core-weibei/config.json`，不在它里面创建 `.creditia/`。
- agent 数据只读打开；证据库不存代码与 prompt。
- 发布（WroughtMind 开源 / npm / Releases）等用户整体验收后由用户发起。

## 3. 目标

### 3.1 新增八个体裁（主题从 7 个到 15 个）

每个体裁有自己的字体、装饰语汇与结构，不是换色。

| id | 名字 | 语汇 | 字体 |
|---|---|---|---|
| `cinema` | 黄金时代片尾 | 黑底金字、Art Deco 双线框、菱形分隔、剧场幕布 | Cinzel / Cinzel Decorative / 站酷小薇 |
| `playbill` | 剧院节目单 | 奶油纸、红黑、Playfair、剧院星、票根虚线 | Playfair Display / Libre Baskerville |
| `gala` | 颁奖礼 | 深蓝金、Art Deco 细线、放射光、大写小字 | Limelight / Poiret One / Oswald |
| `broadsheet` | 老报纸 | 米纸、黑、报头黑字、多栏与花边线 | UnifrakturMaguntia / Playfair / Libre Baskerville |
| `crt` | 老终端 | 绿磷光、扫描线、VT323、框线 | VT323 |
| `arcade` | 街机高分榜 | 深紫、像素框、霓虹色、INSERT COIN | Press Start 2P / 庆科黄油体 |
| `neon` | 霓虹招牌 | 黑、Monoton、粉青发光、灯泡边框 | Monoton |
| `poster` | 海报（包豪斯） | 米白红黑大色块、粗横线、斜切 | Archivo Black / Anton / Bebas Neue |

### 3.2 片尾字幕：五种 → 十一种（真电影语汇）

保留 classic / columns / billing / title / crawl，新增：

- `cast` — 卡司表：左右两列「角色 · 演员」，中缝点线，页脚 in order of appearance
- `scroll` — 双栏滚屏：两列并排向上，中缝竖线，顶部底部渐隐
- `cards` — 卡串：多张卡并列（像漫威片尾一张张概念卡）
- `end` — 终卡：The End + 出品信息，极简收束
- ~~`stinger` — 彩蛋卡~~ → 实现为 `block`（压缩 billing block：presents / production / 片名 / 角色名块 / 合计，宽 940 居中大字块）
- `program` — 剧场节目单：剧院抬头、幕次结构（版本当幕）

### 3.3 小票：四种 → 七种

保留 slip / thermal / folio / stub，新增 `ticket`（电影票/登机牌）、`ledger`（账簿双栏）、`roll`（长条卷纸）。

### 3.4 其他形式家族化

- tier / share / badge / byline：每个新家族一套结构（装饰框、分隔、署名方式）
- 统计图（waffle/donut/area/lanes/clock/versions/settle/mosaic/ribbon）：抽 `statFrame()` 家族边框，避免逐图分支爆炸
- 新增可选 token：`deco`（装饰字体）、`frame`（框线样式）、`pattern`（底纹）

### 3.5 App 重做

主线一条：**作品库 → 图库网格 → 单图聚焦（风格/版式/尺寸 → 复制/保存/嵌入）**，另加档案、设置。
- 砍掉全部解释性文案，只留标签与动作
- 视觉现代：侧栏收起为项目切换器、图库是图不是表单
- 保留：主导者署名开关、嵌入代码、CREDITS.md、doctor、作品库路径

## 4. 任务

- [x] 装字体包，扩展 `index.ts fonts()` 与 App `tools/fonts.mjs` / `src/fonts.ts`（21 个字体，子集已生成）
- [x] `themes.ts`：追加八个主题与新 token（deco / frame / pattern）
- [x] `forms.ts`：tier / share / badge / byline 的新家族分支（tier 走表头×行式×底板配置引擎，badge 八个新画法）
- [x] `forms.ts`：roll 新增六种版式（cast / scroll / cards / end / block / program → 共 11 种）
- [x] `forms.ts`：receipt 新增三种版式（ticket / ledger / scroll → 共 7 种）
- [x] `forms.ts`：统计图 `statFrame()` 家族化（head 八个新家族分支 + plate 底板）
- [x] App：新 IA（图库 → 单图聚焦 → 档案 → 设置），删文案（284 行 → 主线两步拿到图）
- [x] 测试：`packages/render` 6/6、`packages/cli` 3/3 通过；断言 15 主题 / 11 片尾版式 / 7 小票版式
- [x] 样张：`npm run gallery`（修掉 `HOME`/`ROLL_HOME` 缺项后重跑）
- [x] 自查：Playwright 截图逐图看（图库 / 单张 / 档案 / 设置，console 无报错）
- [ ] 实机：`npx tauri build --bundles app` 并启动
- [ ] 文档与记忆更新；stamp 后提交

## 7. 复核记录（2026-10-10）

- 片尾六种新版式拼图复核：`cast` 三列卡司表、`scroll` 双栏中缝加渐隐、`cards` 卡串、`end` 终卡、`program` 幕次节目单一次通过；`block` 首版内容挤在底部、中间空一大块，改成 presents / production / 片名 / 角色名块 / 合计的紧凑居中块后通过。
- 小票三种新版式：`ticket`（票面 + 撕线 + 存根）、`scroll`（长卷纸）通过；`ledger` 首版把纸张容器写成横排 `box`，被压扁并右溢出，改 `col` 后通过。
- badge 八个新画法：`crt` 前缀与数值粘连（加空格）、`neon` 发光偏弱（改为描边光晕 + 纯白字芯）后通过。
- App 复核：齿轮图标改用内联 SVG；图库跳过依赖 CLI 上下文的 `blame`；`profile` 注入全作品库模型；版式 chips 按 `def(t)` 高亮默认版式。

## 5. 验证

- 每个新体裁在 tier / roll / receipt / share / badge 上都有可辨认的结构差异
- 十一种片尾版式逐一看图，构图不重样、不塌陷
- App：从打开到「拿到一张图」不超过两步
- 全部旧主题、旧版式出图与上一轮一致（回归）

## 6. 风险

- 字体多了以后 App 首屏加载变慢 → 字体子集化（tools/fonts.mjs 已在做），中文字体只留 GB2312
- 家族分支让 forms.ts 膨胀 → 用共用 helper（frame/ornament/masthead）收敛
- satori 对 text-shadow / 渐变支持有限 → 发光效果用分层叠加实现
