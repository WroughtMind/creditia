# 候选实现入口与验收依据

`features.json` 固定原桌面和迁移新增的 198 个检查点。下表指向候选实现，便于逐项找到代码；它不是通过清单。每个候选提交的实际结果在 `report.json` 的 `feature_coverage`、`artwork`、`native_catalog`、`browser_catalog`、`performance_comparisons` 和 `package_platforms` 中。`not_run` 表示该次报告没有足够证据，不能推断源码不存在，也不能推断功能可用。

“应用”指 [GPUI 生产视图](../../apps/gpui/src/app.rs)。“服务”指 [桌面共享服务](../../crates/desktop-service/src/)。输入测试必须经过生产控件或系统事件；恢复、设置、读取、写入和取消等检查必须验证实际状态或文件。只使用预先绘制的图片不能验证后台渲染、真实归因或文件保存。

| 清单 ID | 候选实现入口 | 取得什么证据才可判定 |
|---|---|---|
| welcome | 应用 `discover_projects`、`queue_projects`、`start_next_project`、`open_project` | 发现、首选、文件夹选择、批次及逐项失败/取消 |
| project-switcher | 应用 `picker_items`、`move_picker`、`activate_picker`、项目标记缓存 | 实际输入名称/路径、键盘选择、异步更新后焦点 |
| project-row-menu | 应用 `render_panel`、`open_project_settings`、`reveal_project`、`remove_project` | 真实菜单触发、边缘定位、禁用、移除确认 |
| project-open | 应用 `select`、`clear_preview`、`restore_bookmark`；服务 `projects.rs` | 档案戳、切换竞争、错误与未分析状态 |
| aggregate | 应用 `show_all_projects`；服务 `projects.rs` | 多档案合并、跳过不可读档案、项目标签及语言 |
| refresh | 应用 `refresh`、`window_activity`、`start_background`；服务 `background.rs` | 前台/定时变更、Git HEAD、新任务、取消与去重 |
| grouping | 应用 `set_group`、`artwork_request`、`project_preferences`；共享渲染器 | 实际切换、按项目读回、工具/模型数字及文字一致性 |
| wall | 应用 `rebuild_tiles`、`schedule_visible`、`render_stage`；[墙模型](../../apps/gpui/src/ui_model.rs) | 两套作品顺序、真实首屏解码、过期任务、空/错与无障碍 |
| layout | `compute_layout`、`screen_rect`、`ui_model::masonry` | 实际改变窗口宽度，测量跨度、比例、滚动位置 |
| variant | `step_variant`、`choose_variant`、`ui_model::swap_variant` | 前后循环、连续切换、相关墙砖、失败保留原作品 |
| hover | `pointer_move`、`artwork_element`、`screen_rect` | 实际图形/文字命中、平移缩放后命中、边缘与清理 |
| focus | `select_tile`、`select_piece`、`apply_focus_style`、`close_focus`、`can_export` | 真实键盘/点击、试用与应用、导出当前版式、关闭焦点 |
| roam | `sync_roam`、`wheel`、`pointer_move`；[无限漫游模型](../../apps/gpui/src/roam.rs) | 双轴输入、惯性中断、版式轮换、可见挂载及原点 |
| style-picker | `choose_style`、`step_style`、`render_panel`；共享作品目录 | 实际显示顺序/分隔/名称、逐个选择、重启保留 |
| export-menu | `can_export`、`export_request`、`copy_export`、`export_artwork` | 格式可用性、尺寸、复制失败另存、真实输出 |
| export-jobs | `pump_exports`、`retry_export_job`、`cancel_export_job`；[任务队列](../../apps/gpui/src/export_jobs.rs) | 快照、去重、失败恢复、路径、切换项目后完成 |
| drag-export | `pointer_move`、`pointer_up`、`render_export_dock` | 真实拖动到各目标与间隙、改尺寸、取消与清理 |
| analysis | `start_job_target`、`cancel`；服务 `analysis.rs`、`process.rs` | 真实核心/进度、状态、取消子进程、不得重入 |
| analysis-status | `sync_activity`、[动态标记](../../apps/gpui/src/live_mark.rs)、[原生进度](../../apps/gpui/src/platform_ui.rs) | 计时、位置/过渡、任务栏进度、完成/警告入口 |
| warnings | [结构化报告](../../apps/gpui/src/ui_reports.rs)、应用 `render_panel` | 数量与逐条内容、语言、关闭 |
| project-settings | `fetch_details`、`save_project_display`、`link_folder`、`unlink_path` | 通过界面修改真实项目配置，验证覆盖/继承与重试 |
| integrations | `set_integration`、`fetch_details`；服务 `projects.rs` | 安装/卸载真实 hook/workflow、实际路径、失败恢复 |
| remove-readd | `remove_project`、`queue_projects`；服务项目库 | 确认、保留证据及仓库、重新加入、最后项目为空 |
| settings-general | 应用 `render_panel`、`toggle_language`、`persist`；服务 `settings.rs` | 实际更改、重新打开、验证持久值与图标 |
| settings-export | `choose_export_directory`、`set_names`、`export_request` | 真实目录/隐私切换、无效目录错误及恢复 |
| settings-data | `set_collection_mode`、`load_diagnostics`、服务 `collection.rs` | 备份三档、最近结果、平台限制、失败与重试 |
| settings-about | 应用 `render_panel` 的设置内容 | 可见版本/项目/本机运行说明及中英文 |
| native-menu | `install_menus`、`bind_keys`；[平台行为](../../apps/gpui/src/platform_ui.rs) | 各平台原生窗口/编辑命令、系统菜单与语言切换 |
| keyboard | `bind_keys`、`keyboard_navigation`、`open_panel`、`close_panel` | 实际快捷键、局部方向键、输入焦点和空项目状态 |
| language | `toggle_language`、`install_menus`、`init_fonts`、`artwork_request` | 真实菜单/错误/作品/数字日期变化与中文字体 |
| new-zoom | `zoom_at`、`crop_for_art`、`ui_model::texture_width`；[原生预览](../../apps/gpui/src/native_preview.rs) | 缩放锚点、不同 DPI 实际解码/绘制、缓存换代及清晰度 |
| new-theme | `cycle_appearance`、设置持久化与 gpui-kit 主题 | 系统/用户主题、与作品风格独立保存、标题栏 |
| new-reduced-motion | `play`、`toggle_motion`、[播放器](../../apps/gpui/src/animation.rs)、[系统动态偏好](../../apps/gpui/src/platform_ui.rs) | 全作品时间样本及导出、实际播放/暂停/重播/隐藏 |
| new-export-options | 应用 `export_request`、任务快照；[共享导出](../../packages/render/src/export-job.ts) | 各格式尺寸/透明/字体、跨端结果、失败恢复 |
| new-resume | 服务 `analysis.rs`、`export_tasks.rs`、`settings.rs`；GPUI 持久任务 | 进程中断、真实恢复、事务一致性、旧配置回退 |
| new-performance | [资源/场景采集](../../packaging/gpui-acceptance/README.md)、`schedule_visible`、`evict_distant` | 同机同输入的进程树 RSS/CPU、输入与实窗、缓存/去重 |
| new-release | [安装说明](../../packaging/gpui/INSTALL.md)、[发布说明](../../packaging/gpui/RELEASE.md)、[打包流程](../../.github/workflows/gpui.yml) | 四个平台的实际安装、启动、升级/回退及签名状态 |

历史证据按实际运行的候选提交保存。对照旧版时须保留相同输入和不可变基线；相同截图可以用文件哈希证明是此前审阅过的同一张图，但不能把旧候选的行为测试改标为新提交。
