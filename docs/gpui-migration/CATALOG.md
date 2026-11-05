# 基线目录

迁移范围参考主线：`ac82a574b0747bc5651571a3f169f4a893442f88`。以下链接和行号对应本分支捕获时的源码；源码指纹单独记录，不声称等同于该主线提交。目录已登记，GPUI 功能/视觉验收均待验证。

桌面 **26 种风格、25 种作品**；共享注册表合计 28 种作品。台账有 1738 个风格/作品/版式组合，其中 1482 个属于当前桌面可见作品。组合是否有足够输入数据，须运行后判定。

## 26 个桌面风格

| ID | 名称 | 渲染器 | 默认署名榜 / 字幕 | 源码 |
|---|---|---|---|---|
| minimal-light | 极简 | classic | original / classic | [packages/render/src/classic/catalog.ts:7](../../packages/render/src/classic/catalog.ts#L7) |
| minimal-dark | 极简 · 暗 | classic | original / classic | [packages/render/src/classic/catalog.ts:8](../../packages/render/src/classic/catalog.ts#L8) |
| glass | 玻璃 | classic | original / classic | [packages/render/src/classic/catalog.ts:9](../../packages/render/src/classic/catalog.ts#L9) |
| film | 胶片 | classic | original / classic | [packages/render/src/classic/catalog.ts:10](../../packages/render/src/classic/catalog.ts#L10) |
| cinema | 影院 | classic | original / cast | [packages/render/src/classic/catalog.ts:11](../../packages/render/src/classic/catalog.ts#L11) |
| editorial | 杂志 | classic | original / billing | [packages/render/src/classic/catalog.ts:12](../../packages/render/src/classic/catalog.ts#L12) |
| broadsheet | 报纸 | classic | original / columns | [packages/render/src/classic/catalog.ts:13](../../packages/render/src/classic/catalog.ts#L13) |
| playbill | 节目单 | classic | original / program | [packages/render/src/classic/catalog.ts:14](../../packages/render/src/classic/catalog.ts#L14) |
| gala | 颁奖礼 | classic | original / end | [packages/render/src/classic/catalog.ts:15](../../packages/render/src/classic/catalog.ts#L15) |
| mono | 单色 | classic | original / columns | [packages/render/src/classic/catalog.ts:16](../../packages/render/src/classic/catalog.ts#L16) |
| poster | 海报 | classic | original / block | [packages/render/src/classic/catalog.ts:17](../../packages/render/src/classic/catalog.ts#L17) |
| terminal | 终端 | classic | original / columns | [packages/render/src/classic/catalog.ts:18](../../packages/render/src/classic/catalog.ts#L18) |
| crt | 磷光屏 | classic | original / scroll | [packages/render/src/classic/catalog.ts:19](../../packages/render/src/classic/catalog.ts#L19) |
| neon | 霓虹 | classic | original / title | [packages/render/src/classic/catalog.ts:20](../../packages/render/src/classic/catalog.ts#L20) |
| arcade | 街机 | classic | original / cards | [packages/render/src/classic/catalog.ts:21](../../packages/render/src/classic/catalog.ts#L21) |
| ink | 水墨 | svg | rows / vertical | [packages/render/src/styles/east.ts:65](../../packages/render/src/styles/east.ts#L65) |
| ukiyoe | 浮世绘 | svg | rows / vertical | [packages/render/src/styles/east.ts:139](../../packages/render/src/styles/east.ts#L139) |
| woodblock | 古籍 | svg | vertical / vertical | [packages/render/src/styles/east.ts:212](../../packages/render/src/styles/east.ts#L212) |
| baroque | 巴洛克 | svg | podium / classic | [packages/render/src/styles/west.ts:84](../../packages/render/src/styles/west.ts#L84) |
| grimoire | 魔典 | svg | rows / classic | [packages/render/src/styles/west.ts:177](../../packages/render/src/styles/west.ts#L177) |
| manuscript | 手稿 | svg | rows / classic | [packages/render/src/styles/story.ts:12](../../packages/render/src/styles/story.ts#L12) |
| pastel | 对称 | svg | center / card | [packages/render/src/styles/story.ts:140](../../packages/render/src/styles/story.ts#L140) |
| cutout | 剪纸 | svg | rows / billing | [packages/render/src/styles/story.ts:194](../../packages/render/src/styles/story.ts#L194) |
| meadow | 夏日 | svg | rows / ed | [packages/render/src/styles/story.ts:84](../../packages/render/src/styles/story.ts#L84) |
| manga | 漫画 | svg | stats / card | [packages/render/src/styles/pop.ts:29](../../packages/render/src/styles/pop.ts#L29) |
| cyber | 赛博 | svg | hud / terminal | [packages/render/src/styles/cyber.ts:9](../../packages/render/src/styles/cyber.ts#L9) |

SVG 注册表另有 `minimal/noir/glass/cinema/terminal`；桌面为避免重复，只展示相应经典主题。名称相同不能证明内部画法相同，不要删除共享注册表能力。台账含这些共享样式的 215 个额外组合，以 renderer 区分同名主题；desktop=false，不计入桌面验收数量。

## 经典主题作品及全部版式

| 作品 | 名称 | 版式 | 桌面 | 可生成动画帧 | 源码 |
|---|---|---|---|---|---|
| tier | 署名榜 | original, rows, center, podium, vertical, hud, stats, split, grouped | 是 | 是 | [packages/render/src/classic/registry.ts:13](../../packages/render/src/classic/registry.ts#L13) |
| share | 分享图 | 默认 | 是 | 是 | [packages/render/src/classic/registry.ts:14](../../packages/render/src/classic/registry.ts#L14) |
| byline | 署名行 | original, models | 是 | 是 | [packages/render/src/classic/registry.ts:15](../../packages/render/src/classic/registry.ts#L15) |
| receipt | 小票 | slip, thermal, folio, stub, ticket, ledger, scroll, themed, itemized | 是 | 是 | [packages/render/src/classic/registry.ts:16](../../packages/render/src/classic/registry.ts#L16) |
| roll | 片尾字幕 | classic, columns, billing, title, crawl, cast, scroll, cards, end, block, program, opening, production, slate, crew, card, vertical, ed, terminal, troupe | 是 | 是 | [packages/render/src/classic/registry.ts:18](../../packages/render/src/classic/registry.ts#L18) |
| ribbon | 时间织带 | 默认 | 是 | 是 | [packages/render/src/classic/registry.ts:19](../../packages/render/src/classic/registry.ts#L19) |
| mosaic | 代码马赛克 | 默认 | 是 | 是 | [packages/render/src/classic/registry.ts:20](../../packages/render/src/classic/registry.ts#L20) |
| profile | 个人履历卡 | 默认 | 共享模块额外能力 | 是 | [packages/render/src/classic/registry.ts:21](../../packages/render/src/classic/registry.ts#L21) |
| fetch | 终端名片 | 默认 | 是 | 是 | [packages/render/src/classic/registry.ts:22](../../packages/render/src/classic/registry.ts#L22) |
| blame | 溯源 | 默认 | 共享模块额外能力 | 是 | [packages/render/src/classic/registry.ts:23](../../packages/render/src/classic/registry.ts#L23) |
| app | App 致谢页 | ios, themed | 是 | 是 | [packages/render/src/classic/registry.ts:24](../../packages/render/src/classic/registry.ts#L24) |
| waffle | 华夫饼图 | original, blocks | 是 | 是 | [packages/render/src/classic/registry.ts:25](../../packages/render/src/classic/registry.ts#L25) |
| donut | 环形图 | original, nested | 是 | 是 | [packages/render/src/classic/registry.ts:26](../../packages/render/src/classic/registry.ts#L26) |
| area | 堆叠面积图 | 默认 | 是 | 是 | [packages/render/src/classic/registry.ts:27](../../packages/render/src/classic/registry.ts#L27) |
| lanes | 泳道图 | 默认 | 是 | 是 | [packages/render/src/classic/registry.ts:28](../../packages/render/src/classic/registry.ts#L28) |
| clock | 作息图 | 默认 | 是 | 是 | [packages/render/src/classic/registry.ts:29](../../packages/render/src/classic/registry.ts#L29) |
| versions | 版本柱状图 | 默认 | 是 | 是 | [packages/render/src/classic/registry.ts:30](../../packages/render/src/classic/registry.ts#L30) |
| settle | 沉淀 | 默认 | 是 | 是 | [packages/render/src/classic/registry.ts:31](../../packages/render/src/classic/registry.ts#L31) |
| badge | 徽章 | original, flat, stack, emblem, split | 是 | 否 | [packages/render/src/classic/bridge.ts:188](../../packages/render/src/classic/bridge.ts#L188) |
| stream | 河流图 | 默认 | 是 | 是 | [packages/render/src/classic/bridge.ts:183](../../packages/render/src/classic/bridge.ts#L183) |
| survival | 留存 | 默认 | 是 | 是 | [packages/render/src/classic/bridge.ts:183](../../packages/render/src/classic/bridge.ts#L183) |
| evidence | 出处 | 默认 | 是 | 是 | [packages/render/src/classic/bridge.ts:183](../../packages/render/src/classic/bridge.ts#L183) |
| areas | 板块 | 默认 | 是 | 是 | [packages/render/src/classic/bridge.ts:183](../../packages/render/src/classic/bridge.ts#L183) |
| codemap | 代码地图 | 默认 | 是 | 是 | [packages/render/src/classic/bridge.ts:183](../../packages/render/src/classic/bridge.ts#L183) |
| calendar | 日历 | 默认 | 是 | 是 | [packages/render/src/classic/bridge.ts:183](../../packages/render/src/classic/bridge.ts#L183) |
| asterisk | 星号 | 默认 | 是 | 是 | [packages/render/src/classic/bridge.ts:183](../../packages/render/src/classic/bridge.ts#L183) |
| people | 贡献者 | 默认 | 是 | 是 | [packages/render/src/classic/bridge.ts:183](../../packages/render/src/classic/bridge.ts#L183) |

## SVG 风格作品及全部版式

| 作品 | 名称 | 版式 | 桌面 | 可生成动画帧 | 源码 |
|---|---|---|---|---|---|
| tier | 署名榜 | rows, center, podium, vertical, hud, stats, split, grouped | 是 | 是 | [packages/render/src/registry.ts:29](../../packages/render/src/registry.ts#L29) |
| asterisk | 星号 | 默认 | 是 | 是 | [packages/render/src/registry.ts:30](../../packages/render/src/registry.ts#L30) |
| share | 分享图 | 默认 | 是 | 是 | [packages/render/src/registry.ts:31](../../packages/render/src/registry.ts#L31) |
| hand | 开发者 | 默认 | 共享模块额外能力 | 是 | [packages/render/src/registry.ts:32](../../packages/render/src/registry.ts#L32) |
| people | 贡献者 | 默认 | 是 | 是 | [packages/render/src/registry.ts:33](../../packages/render/src/registry.ts#L33) |
| receipt | 小票 | slip, ticket, itemized | 是 | 否 | [packages/render/src/registry.ts:34](../../packages/render/src/registry.ts#L34) |
| badge | 徽章 | flat, stack, emblem, split | 是 | 否 | [packages/render/src/registry.ts:35](../../packages/render/src/registry.ts#L35) |
| byline | 署名行 | line, models | 是 | 否 | [packages/render/src/registry.ts:36](../../packages/render/src/registry.ts#L36) |
| roll | 片尾字幕 | classic, scroll, card, vertical, ed, billing, terminal, troupe | 是 | 是 | [packages/render/src/registry.ts:37](../../packages/render/src/registry.ts#L37) |
| donut | 环形图 | ring, nested | 是 | 是 | [packages/render/src/registry.ts:38](../../packages/render/src/registry.ts#L38) |
| waffle | 华夫饼图 | grid, blocks | 是 | 是 | [packages/render/src/registry.ts:39](../../packages/render/src/registry.ts#L39) |
| stream | 河流图 | 默认 | 是 | 是 | [packages/render/src/registry.ts:40](../../packages/render/src/registry.ts#L40) |
| lanes | 会话泳道 | 默认 | 是 | 是 | [packages/render/src/registry.ts:41](../../packages/render/src/registry.ts#L41) |
| clock | 作息打卡 | 默认 | 是 | 是 | [packages/render/src/registry.ts:42](../../packages/render/src/registry.ts#L42) |
| versions | 版本柱状图 | 默认 | 是 | 是 | [packages/render/src/registry.ts:43](../../packages/render/src/registry.ts#L43) |
| survival | 留存 | 默认 | 是 | 是 | [packages/render/src/registry.ts:44](../../packages/render/src/registry.ts#L44) |
| mosaic | 文件马赛克 | 默认 | 是 | 是 | [packages/render/src/registry.ts:45](../../packages/render/src/registry.ts#L45) |
| calendar | 日历 | 默认 | 是 | 是 | [packages/render/src/registry.ts:46](../../packages/render/src/registry.ts#L46) |
| evidence | 出处 | 默认 | 是 | 是 | [packages/render/src/registry.ts:47](../../packages/render/src/registry.ts#L47) |
| areas | 板块 | 默认 | 是 | 是 | [packages/render/src/registry.ts:48](../../packages/render/src/registry.ts#L48) |
| codemap | 代码地图 | 默认 | 是 | 是 | [packages/render/src/registry.ts:49](../../packages/render/src/registry.ts#L49) |

`profile` 需要项目集合与个人上下文；`blame` 需要溯源结果；`hand` 未列入默认桌面展墙。台账仍保留这些注册能力。经典主题追加版式经 `classic/bridge.ts` 使用其自己的颜色、字体及框线；不等于直接套用 SVG 风格。

## 全局快捷键

| 操作 | macOS / Windows、Linux | 源码 |
|---|---|---|
| 切换项目… | CmdOrCtrl+K | [apps/desktop/src/main.ts:1501](../../apps/desktop/src/main.ts#L1501) |
| 添加文件夹… | CmdOrCtrl+O | [apps/desktop/src/main.ts:1502](../../apps/desktop/src/main.ts#L1502) |
| 重新分析 | CmdOrCtrl+R | [apps/desktop/src/main.ts:1503](../../apps/desktop/src/main.ts#L1503) |
| 项目设置… | CmdOrCtrl+I | [apps/desktop/src/main.ts:1504](../../apps/desktop/src/main.ts#L1504) |
| 设置… | CmdOrCtrl+, | [apps/desktop/src/main.ts:1505](../../apps/desktop/src/main.ts#L1505) |
| 墙 | CmdOrCtrl+1 | [apps/desktop/src/main.ts:1506](../../apps/desktop/src/main.ts#L1506) |
| 漫游 | CmdOrCtrl+2 | [apps/desktop/src/main.ts:1507](../../apps/desktop/src/main.ts#L1507) |
| 上一种风格 | CmdOrCtrl+[ | [apps/desktop/src/main.ts:1508](../../apps/desktop/src/main.ts#L1508) |
| 下一种风格 | CmdOrCtrl+] | [apps/desktop/src/main.ts:1509](../../apps/desktop/src/main.ts#L1509) |
| 导出… | CmdOrCtrl+E | [apps/desktop/src/main.ts:1510](../../apps/desktop/src/main.ts#L1510) |

## 页面和操作

| 功能 | 阶段 | 基线 | 检查项 |
|---|---|---|---|
| 首次打开与项目发现 | 4 | [apps/desktop/src/main.ts:1838](../../apps/desktop/src/main.ts#L1838) | 扫描本地含 AI 记录的 Git 仓库；默认选择前三个候选项目；逐个选择或批量分析；手动选择文件夹；扫描中、无候选、扫描失败与重试；逐项运行、完成、失败、取消状态 |
| 项目切换与搜索 | 4 | [apps/desktop/src/main.ts:1571](../../apps/desktop/src/main.ts#L1571) | 按名称和路径搜索；已加入、尚未添加、全部项目分区；目录丢失和上次分析日期；后台发现与星号补齐时保留焦点及更多菜单；方向键选择、回车打开、点击外部关闭 |
| 项目更多菜单 | 4 | [apps/desktop/src/main.ts:1673](../../apps/desktop/src/main.ts#L1673) | 重新分析；在系统文件管理器中打开；项目设置；移除前确认；已删除目录禁用不适用操作；菜单边缘避让 |
| 打开和切换档案 | 3 | [apps/desktop/src/main.ts:313](../../apps/desktop/src/main.ts#L313) | 读取项目档案及版本戳；丢弃过期项目请求；释放旧漫游预览与排版任务；未分析提示与立即分析；读取失败可理解的错误状态；保留项目名称与星号缓存 |
| 全部项目汇总 | 3 | [apps/desktop/src/main.ts:366](../../apps/desktop/src/main.ts#L366) | 多个项目时显示全部项目入口；合并档案且文件和 PR 带项目名；无法读取的档案遵循现有跳过规则；工具和模型分组；汇总名称跟随语言 |
| 外部变更和自动增量更新 | 3 | [apps/desktop/src/main.ts:373](../../apps/desktop/src/main.ts#L373) | 窗口回到前台、可见性变化和定时检查；比较档案版本和 Git HEAD；新提交触发增量分析；保持滚动位置与已选版式；后台任务避免重复运行；项目级设置覆盖全局默认 |
| 工具与模型切换 | 4 | [apps/desktop/src/main.ts:474](../../apps/desktop/src/main.ts#L474) | 自动、工具、模型分组语义；没有模型记录时隐藏切换；逐项目记住偏好；作品、文字输出、星号与悬停数据一致 |
| 展墙 | 4 | [apps/desktop/src/main.ts:546](../../apps/desktop/src/main.ts#L546) | 保持两套作品顺序、重复徽章版式和跨列布局；占位与首屏渐进绘制；只发布当前项目和风格的结果；空作品和排版错误处理；交互区域及无障碍标签；中英文尺寸缓存 |
| 响应式展墙排版 | 4 | [apps/desktop/src/main.ts:669](../../apps/desktop/src/main.ts#L669) | 窗口改变尺寸后重新排版；宽作品 span；按作品原始纵横比排布；最小窗口尺寸；滚动与标题区状态 |
| 版式切换 | 4 | [apps/desktop/src/main.ts:755](../../apps/desktop/src/main.ts#L755) | 前后版式循环；风格默认版式；连续点击只应用最后一次；同作品相关墙砖同步；失败保持可用旧作品 |
| 作品悬停详情 | 4 | [apps/desktop/src/main.ts:1216](../../apps/desktop/src/main.ts#L1216) | 文字和图形命中区域；比例与值的语义保持；屏幕边缘避让；平移和缩放后命中准确；失焦和销毁时清理浮层 |
| 作品放大视图 | 4 | [apps/desktop/src/main.ts:997](../../apps/desktop/src/main.ts#L997) | 点击及 Enter/Space 打开；上下切作品、左右切版式；局部试用其它支持该作品的风格；设为全局风格；输出正在看的作品和版式；加载时禁用导出；失败重试；关闭后恢复原键盘焦点 |
| 漫游画布 | 4 | [apps/desktop/src/main.ts:1321](../../apps/desktop/src/main.ts#L1321) | 鼠标拖动、滚轮和触控板双轴移动；Shift 滚轮横移；惯性滑动与中断；每轮切换作品版式；仅挂载可见区附近作品；点击放大及悬停导出；方向键移动、Home/0 回原点；重进漫游和窗口缩放的位置保持 |
| 风格选择 | 4 | [apps/desktop/src/main.ts:1424](../../apps/desktop/src/main.ts#L1424) | 26 个桌面风格及顺序；15 个经典主题与新风格间分隔；逐个前后切换；风格圆点和名称；关闭及边缘定位；记住上次风格 |
| 作品导出菜单 | 7 | [apps/desktop/src/main.ts:801](../../apps/desktop/src/main.ts#L801) | PNG/SVG/PDF/GIF/HTML 与复制 Markdown；仅动画作品可选 GIF；记住格式和尺寸；原尺寸、横图、方图、竖图、竖屏；复制失败可另存 Markdown 文件 |
| 导出任务 | 7 | [apps/desktop/src/main.ts:833](../../apps/desktop/src/main.ts#L833) | 运行、已保存、失败计数；相同输入和选项去重；任务名包含作品、版式、风格与尺寸；切换项目后任务仍完成到原请求；显示保存位置、系统定位、关闭完成任务；错误不吞掉、允许再次发起 |
| 拖放导出坞 | 4 | [apps/desktop/src/main.ts:935](../../apps/desktop/src/main.ts#L935) | 拖起作品显示预览和导出坞；PNG/SVG/PDF/HTML 目标；拖动中选择尺寸；格式按钮间缝隙按最近目标；取消和释放清理；轻点仍打开放大视图 |
| 分析任务与取消 | 3 | [apps/desktop/src/main.ts:1748](../../apps/desktop/src/main.ts#L1748) | 真实 Rust 核心分析与结构化进度；等待、分析中、取消中、完成、失败；前台阻塞与后台分析区分；停止子进程及收尾；不允许重入；取消后可再次分析 |
| 分析状态与星号 | 4 | [apps/desktop/src/main.ts:195](../../apps/desktop/src/main.ts#L195) | 300ms 和 2s 的延迟呈现；顶部与中央星号位置；每瓣份额与加载、进度、错误动作；程序坞或任务栏进度同步；完成提示与警告入口 |
| 分析警告 | 4 | [apps/desktop/src/main.ts:1896](../../apps/desktop/src/main.ts#L1896) | 结构化警告数量；逐条查看分析问题；关闭和语言同步 |
| 项目设置 | 4 | [apps/desktop/src/main.ts:1914](../../apps/desktop/src/main.ts#L1914) | 上次分析及重新分析；跟随默认、自动跟上、关闭；Git 提交盖章和发版/PR 工作流接入；接入状态失败重试；主目录、自动发现、手动关联路径；关联和解除关联文件夹；移除前确认 |
| 提交与发布接入 | 3 | [apps/desktop/src/main.ts:457](../../apps/desktop/src/main.ts#L457) | 安装及卸载提交 hook；安装及卸载发布工作流；显示实际会修改的位置；非 GitHub 仓库状态；已发布档案与 hook 依赖提示；忙碌和失败恢复 |
| 移除及重新加入 | 3 | [apps/desktop/src/main.ts:1703](../../apps/desktop/src/main.ts#L1703) | 只移除列表记录；保留本地证据、缓存和仓库文件；可立即加回来；当前项目和全部项目视图正确更新；最后一个项目移除后回首次打开 |
| 通用设置 | 2 | [apps/desktop/src/main.ts:1985](../../apps/desktop/src/main.ts#L1985) | 中英文；程序坞图标选择；自动跟上新提交默认值；偏好持久化 |
| 导出设置 | 7 | [apps/desktop/src/main.ts:1985](../../apps/desktop/src/main.ts#L1985) | 选择保存目录；恢复默认目录；打开导出目录；是否显示其他贡献者姓名；默认目录不可用时的错误 |
| 数据设置 | 3 | [apps/desktop/src/main.ts:1985](../../apps/desktop/src/main.ts#L1985) | 显示并打开项目库位置；关闭、App 打开时、一直备份三档；macOS 后台任务支持范围；备份运行和最近结果；AI 工具诊断及不同来源状态；诊断失败可重试 |
| 关于 | 2 | [apps/desktop/src/main.ts:1985](../../apps/desktop/src/main.ts#L1985) | 应用版本；项目说明；本机运行说明 |
| 系统菜单与窗口 | 2 | [apps/desktop/src/main.ts:1532](../../apps/desktop/src/main.ts#L1532) | macOS 应用、编辑、项目、显示、窗口菜单；剪切、复制、粘贴、全选及撤销重做；系统隐藏、退出、全屏、最小化、关闭；菜单快捷键与 UI 命令共用；语言切换重建菜单 |
| 快捷键与焦点 | 2 | [apps/desktop/src/main.ts:1522](../../apps/desktop/src/main.ts#L1522) | 十个 Cmd/Ctrl 命令；局部方向键与 Escape；输入框不被漫游方向键抢占；对话框关闭后焦点返回；欢迎页只允许可用命令 |
| 中英文切换 | 2 | [apps/desktop/src/main.ts:1481](../../apps/desktop/src/main.ts#L1481) | 菜单、设置、导出、错误、提示同步；作品文字与历史标签同步；数字和日期格式；中文字体覆盖 |
| 连续缩放与像素密度 | 4 | 本次新增要求 | GPUI 画布连续缩放；缩放锚点和拖动坐标一致；不同 DPI 字体及纹理清晰；缓存按缩放档和像素密度失效 |
| 系统明暗主题 | 2 | 本次新增要求 | 应用外壳跟随系统或用户选择；外壳主题与作品风格分别持久化；原生标题栏及控件规范 |
| 完整动作定义与减少动态效果 | 6 | 本次新增要求 | 26 风格逐件动作审定；星号逐瓣、有机形变、水墨展开、片尾滚动；桌面与动画导出同一时间轴；连续操作、中断、重播；遵循系统及应用减少动态效果设置；不可见动画暂停 |
| 透明背景、字体与失败恢复 | 7 | 本次新增要求 | 明确各格式透明背景支持；字体嵌入或转路径策略及许可证；导出进度与可重试失败；恢复不覆盖错误输入或旧任务；桌面、CLI、网页的尺寸和数据一致 |
| 中断后恢复 | 3 | 本次新增要求 | 进程退出后恢复持久任务状态；部分结果和事务的一致性；取消后不继续写入；已有数据库和配置可回退 |
| 性能与资源基准 | 8 | 本次新增要求 | 相同设备、数据、作品和布局规模；冷/热启动、滚动、缩放、交互延迟；整组进程 RSS、CPU、GPU 资源；空闲 CPU 与不可见动画；后台排版和渲染；纹理缓存上限和释放；请求去重及过期任务丢弃 |
| 跨平台发布 | 9 | 本次新增要求 | macOS Apple Silicon 与 Intel；Windows 与 Linux；最低系统和图形依赖；独立安装包与升级；签名、公证和发布 CI；升级备份及回退；完整验收报告和维护文档 |

## 输出边界

| 格式 | 当前桌面 | 拖放坞 | 说明 |
|---|---|---|---|
| PNG | 有 | 有 | 共享导出器 |
| SVG | 有 | 有 | 共享导出器 |
| PDF | 有 | 有 | 共享导出器 |
| GIF | 有 | 无 | animated artwork only |
| HTML | 有 | 有 | 共享导出器 |
| Markdown | 有 | 无 | clipboard with file fallback |
| 嵌入 | 隐藏 | 无 | shared declaration; hidden in desktop/CLI list |

- JSON、Markdown 文档、版本说明、PR/提交归因文字、动态徽章数据、终端名片、HTML 展墙及站点另由 CLI 提供。
- 尺寸：native、og 1200×630、square 1080×1080、portrait 1080×1350、story 1080×1920、a4 2480×3508；A4 当前仅共享/CLI，桌面隐藏；CLI 支持自定义宽高。
- SVG/HTML 文字可转路径；PDF 当前为带透明蒙版的栅格图像 PDF，不能宣称已实现可编辑矢量 PDF 或字体嵌入。
- GIF 使用已有帧生成器；完整桌面动态播放与风格动作仍需迁移和补齐。开放的 [#116](https://github.com/WroughtMind/creditia/issues/116) 是待完成要求，不计为已交付基线。
- `fixtures/catnap.json` 是虚构样例；真实回放样例是有限的归因输入，不是完整视觉档案。真实项目视觉样例和截图仍待采集。
