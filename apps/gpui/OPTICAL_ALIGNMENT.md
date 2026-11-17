# 光学对齐

参考：[枯白啃设计分享的视频](https://x.com/Kubai087/status/2122514434069537097)。视频先处理飞机图标，再处理「47m」文字：模糊细节，让视觉体积显现，参照胶囊内轮廓调整位置，最后恢复清晰。

## 桌面控件

- 图标保留 16 × 16 的布局框、1.5 的描边和原有图形。仅在 SVG 内移动有偏差的笔画，按钮的焦点、悬停和点击区域随布局框保持稳定。
- 文件夹、刷新、导出、复制、删除、勾选和成对箭头分别校准。对称图标不追加偏移。不要对整行控件统一做 `translate`。
- 顶栏和浮动栏的图标按钮统一为 32 × 32；胶囊保留 4 的内边距。缩放加减使用相同描边的矢量图形，百分比按钮固定宽度，避免缩放过程中相邻控件移动。
- 图标按钮同时提供中文、英文的无障碍名称和悬停说明。文本继续由字体排版，不按单一语言的基线统一平移。

光学修正存放在 `src/ui_icons.rs` 的实际 SVG 中，控件尺寸存放在 `src/ui_controls.rs`。切换主题、禁用、悬停和焦点状态都使用同一图形。

## App 面板

- 设置使用统一的 40px 整行控件：左侧名称、右侧当前值或开关。开关提供 Switch 角色和开关状态，沿用原有回调及禁用条件；外观值使用当前语言。应用标志显示六个实际资源，缓存图片身份以保证预览能够完成加载。
- 项目切换的图标纳入整行点击区域，缺少图标时显示文件夹。项目名称允许截断，时间保留固定位置；更多操作具有明确的操作名称。
- 风格选择保留全部风格，统一预览、名称和勾选位置。布局选择与导出格式、尺寸等选择项同时提供选中状态；导出尺寸使用可读名称，透明背景使用复选框。
- 项目署名输入增加可见名称，面板动作统一左对齐。弹窗与导出栏使用同一关闭图标按钮。作品渲染器和各风格的作品设计不在这些调整范围内。

可用现有 fixture 捕获五个实际 App 表面：

```sh
cargo run --manifest-path apps/gpui/Cargo.toml --features visual-test \
  --example optical-capture -- \
  evidence/optical-fixtures/minimal-light-zh.json evidence/app-ui
```

程序等待实际预览资源解码，依次点击打开设置、项目库、风格和项目设置，再通过原有快捷键打开导出栏。设置开关同时验证点击以及 Tab 后的 Space 激活。截图覆盖原生像素和这些导航操作，不代表文件导出、持久化和所有交互均已验收。

## 复核方法

```sh
npm ci --prefix packages/render
node packaging/gpui/optical-review.mjs --baseline origin/main
```

打开 `evidence/optical-review/index.html`：在浅、深两种表面上并排检查修改前后、清晰与模糊视图。参考线可关闭，观察实际体感。生成的 `metrics.json` 记录墨迹重心、面积和边缘检查；重心只是辅助判断，不能代替对图形语义和完整控件的观察。

原生窗口沿用现有的代表性截图和输入验证：

需要单独复核一张图时，增加 `CREDITIA_VISUAL_CASE=minimal-light-zh-dark-wall` 等具体名称。报告会标记为 `single-case`，不代表整套截图检查通过。

```sh
node packaging/gpui/fixtures.mjs --output evidence/optical-fixtures --profile smoke
CREDITIA_VISUAL_FIXTURES="$PWD/evidence/optical-fixtures" \
CREDITIA_VISUAL_OUTPUT="$PWD/evidence/optical-native-after" \
cargo test --manifest-path apps/gpui/Cargo.toml --features visual-test --test rendering --locked
```

保存基线提交的原生截图后，可将两组真实窗口加入对照页：

```sh
node packaging/gpui/optical-review.mjs --baseline origin/main \
  --native-before evidence/optical-native-before \
  --native-after evidence/optical-native-after
```

对照页会明确区分矢量诊断与原生截图；未提供原生截图时不会宣称已完成窗口验证。模糊仅用于复核，不进入产品界面。
