//! One in-window export workspace, with a preview of the exact export settings.
use super::*;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ExportFormat {
    Png,
    Svg,
    Pdf,
    Gif,
    Html,
    Embed,
    Md,
    Txt,
    Json,
}
impl ExportFormat {
    const ALL: [Self; 9] = [
        Self::Png,
        Self::Svg,
        Self::Pdf,
        Self::Gif,
        Self::Html,
        Self::Embed,
        Self::Md,
        Self::Txt,
        Self::Json,
    ];
    pub(super) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|format| format.id() == id)
    }
    fn id(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
            Self::Pdf => "pdf",
            Self::Gif => "gif",
            Self::Html => "html",
            Self::Embed => "embed",
            Self::Md => "md",
            Self::Txt => "txt",
            Self::Json => "json",
        }
    }
    fn group(self) -> usize {
        match self {
            Self::Png | Self::Svg | Self::Pdf => 0,
            Self::Gif | Self::Html | Self::Embed => 1,
            _ => 2,
        }
    }
    fn visual(self) -> bool {
        matches!(
            self,
            Self::Png | Self::Svg | Self::Pdf | Self::Gif | Self::Html
        )
    }
    pub(super) fn transparency(self) -> bool {
        matches!(self, Self::Png | Self::Svg)
    }
    fn fonts(self) -> bool {
        matches!(self, Self::Svg | Self::Html)
    }
    fn icon(self) -> &'static str {
        match self {
            Self::Png => "image",
            Self::Svg => "pen",
            Self::Pdf | Self::Md => "file",
            Self::Gif => "film",
            Self::Html => "globe",
            Self::Embed => "code",
            Self::Txt => "text",
            Self::Json => "json",
        }
    }
    fn label(self, en: bool) -> &'static str {
        let (zh, en_label) = match self {
            Self::Png => ("PNG 图像", "PNG image"),
            Self::Svg => ("SVG 矢量图", "SVG vector"),
            Self::Pdf => ("PDF 文档", "PDF document"),
            Self::Gif => ("GIF 动图", "GIF animation"),
            Self::Html => ("HTML 网页", "HTML page"),
            Self::Embed => ("嵌入组件", "Embed"),
            Self::Md => ("CREDITS.md", "CREDITS.md"),
            Self::Txt => ("TXT 纯文本", "Plain text"),
            Self::Json => ("JSON 数据", "JSON data"),
        };
        if en { en_label } else { zh }
    }
    fn description(self, en: bool) -> &'static str {
        let (zh, en_label) = match self {
            Self::Png => ("分享图片", "Share an image"),
            Self::Svg => ("编辑与排版", "Edit and scale"),
            Self::Pdf => ("打印与归档", "Print and archive"),
            Self::Gif => ("保留动态", "Keep the motion"),
            Self::Html => ("独立打开", "Open as a web page"),
            Self::Embed => ("放入网页", "Add to a website"),
            Self::Md => ("Markdown", "Markdown"),
            Self::Txt => ("直接粘贴", "Paste anywhere"),
            Self::Json => ("程序读取", "Structured data"),
        };
        if en { en_label } else { zh }
    }
}

#[derive(Default)]
pub(super) struct ExportPreview {
    revision: u64,
    key: String,
    task: Option<Task<()>>,
    cancellation: Option<renderer::Cancellation>,
    image: Option<Arc<Image>>,
    image_ready: bool,
    text: Option<String>,
    lines: Arc<Vec<SharedString>>,
    error: Option<String>,
}
impl ExportPreview {
    pub(super) fn clear(&mut self, cx: &mut App) {
        if let Some(cancel) = self.cancellation.take() {
            cancel.cancel();
        }
        self.task = None;
        if let Some(image) = self.image.take() {
            crate::image_assets::release(image, cx);
        }
        let revision = self.revision.wrapping_add(1);
        *self = Self {
            revision,
            ..Self::default()
        };
    }
    pub(super) fn ready(&self) -> bool {
        self.error.is_some() || self.text.is_some() || self.image_ready
    }
    pub(super) fn diagnostic(&self) -> Value {
        json!({"busy":self.task.is_some(),"image_ready":self.image_ready,"text_ready":self.text.is_some(),"error":self.error,"revision":self.revision})
    }
}

fn export_text(format: ExportFormat, bytes: Vec<u8>) -> Result<String, String> {
    let text = String::from_utf8(bytes).map_err(|error| error.to_string())?;
    Ok(if format == ExportFormat::Md {
        format!(
            "# Credits\n\n<!-- creditia:start -->\n\n{}\n\n<!-- creditia:end -->\n",
            text.trim_end()
        )
    } else {
        text
    })
}

impl Creditia {
    pub(super) fn export_workspace_open(&self) -> bool {
        self.export_open
            && !self
                .drag
                .as_ref()
                .is_some_and(|drag| drag.moved && drag.tile.is_some())
    }
    pub(super) fn close_export_workspace(&mut self, cx: &mut Context<Self>) {
        self.export_open = false;
        self.export_preview.clear(cx);
        self.hover = None;
        cx.notify();
    }
    fn choose_export_format(&mut self, format: ExportFormat, cx: &mut Context<Self>) {
        if self.export_prompt_busy || self.export_unavailable_reason(format.id()).is_some() {
            return;
        }
        self.export_format = format.id().into();
        self.preferences.export_format = self.export_format.clone();
        self.notice = None;
        self.persist();
        cx.notify();
    }
    pub(super) fn refresh_export_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.export_workspace_open() {
            if !self.export_preview.key.is_empty() {
                self.export_preview.clear(cx);
            }
            return;
        }
        if let Some(image) = &self.export_preview.image {
            self.export_preview.image_ready =
                crate::image_assets::track(image, window, cx).is_some();
            if !self.export_preview.image_ready {
                window.request_animation_frame();
            }
        }
        let Some(format) = ExportFormat::from_id(&self.export_format) else {
            return;
        };
        let Some(tile) = self.current_tile() else {
            return;
        };
        let scale = if format.visual() {
            let Some(art) = tile.art.as_ref() else {
                if let Some(error) = &tile.error {
                    self.export_preview.error = Some(if error == "__empty__" {
                        self.text("此作品没有可预览的内容，可选择左侧的文字与数据。","This artwork has no preview content. Text and data are available on the left.").into()
                    } else {
                        error.clone()
                    });
                }
                return;
            };
            let size = self
                .catalog
                .as_ref()
                .and_then(|catalog| catalog["sizes"].as_array())
                .and_then(|sizes| sizes.iter().find(|size| size["id"] == self.export_size));
            let (out_w, out_h) = size
                .filter(|size| size["w"].as_f64().unwrap_or(0.) > 0.)
                .map(|size| (size["w"].as_f64().unwrap(), size["h"].as_f64().unwrap()))
                .unwrap_or((art.w as f64, art.h as f64));
            let preview_w =
                (self.viewport_size.0 - self.export_sidebar_width() - 48.).max(1.) as f64;
            let preview_h = (self.viewport_size.1 - 216.).max(1.) as f64;
            ((preview_w / out_w).min(preview_h / out_h) * self.display_scale as f64).min(8.)
        } else {
            1.
        };
        let key = format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            self.preview_generation,
            tile.identity,
            self.export_format,
            self.export_size,
            self.transparent,
            self.font_mode,
            self.preferences.language.code(),
            self.group(),
            self.preferences.named,
            scale.to_bits()
        );
        if self.export_preview.key == key {
            return;
        }
        self.export_preview.clear(cx);
        self.export_preview.key = key.clone();
        let revision = self.export_preview.revision;
        let mut request =
            match self.export_request_format(if format.visual() { "png" } else { format.id() }) {
                Ok(request) => request,
                Err(error) => {
                    self.export_preview.error = Some(error);
                    return;
                }
            };
        request["transparent"] = json!(self.transparent && format.transparency());
        if format.visual() {
            request["scale"] = json!(scale);
        }
        let cancel = renderer::Cancellation::default();
        self.export_preview.cancellation = Some(cancel.clone());
        self.export_preview.task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let result = renderer::request(request, cancel)?;
                    renderer::decode_export(
                        &result,
                        if format.visual() { "png" } else { format.id() },
                    )
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                if !view.export_workspace_open()
                    || view.export_preview.key != key
                    || view.export_preview.revision != revision
                {
                    return;
                }
                view.export_preview.task = None;
                match result {
                    Ok(bytes) if format.visual() => {
                        view.export_preview.image =
                            Some(Arc::new(Image::from_bytes(ImageFormat::Png, bytes)))
                    }
                    Ok(bytes) => match export_text(format, bytes) {
                        Ok(text) => {
                            view.export_preview.lines = Arc::new(
                                text.lines()
                                    .map(|line| SharedString::from(line.to_owned()))
                                    .collect(),
                            );
                            view.export_preview.text = Some(text);
                        }
                        Err(error) => view.export_preview.error = Some(error),
                    },
                    Err(error) => view.export_preview.error = Some(error),
                }
                cx.notify();
            });
        }));
    }
    fn export_sidebar_width(&self) -> f32 {
        if self.viewport_size.0 < 1000. {
            224.
        } else {
            255.
        }
    }
    fn export_size_label(&self, id: &str) -> String {
        match id {
            "native" => self.text("原尺寸", "Original size"),
            "og" => self.text("链接预览", "Link preview"),
            "square" => self.text("方图", "Square"),
            "portrait" => self.text("竖图", "Portrait"),
            "story" => self.text("竖屏", "Story"),
            "a4" => "A4",
            _ => id,
        }
        .to_owned()
    }
    pub(super) fn render_export_workspace(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.export_workspace_open() {
            return div().into_any_element();
        }
        let p = Palette::of(cx);
        let en = self.preferences.language == Language::En;
        let format = ExportFormat::from_id(&self.export_format).unwrap_or(ExportFormat::Png);
        let mut sidebar = div()
            .id("export-formats")
            .flex()
            .flex_col()
            .min_h_0()
            .overflow_y_scroll()
            .w(px(self.export_sidebar_width()))
            .flex_shrink_0()
            .p(px(16.))
            .border_r_1()
            .border_color(p.edge)
            .bg(p.ink.opacity(0.025));
        for (group, title) in [
            self.text("图像与文档", "Images & documents"),
            self.text("动态与网页", "Motion & web"),
            self.text("文字与数据", "Text & data"),
        ]
        .into_iter()
        .enumerate()
        {
            let mut section = div().flex().flex_col().gap(px(2.)).mb(px(16.)).child(
                div()
                    .px(px(10.))
                    .mb(px(6.))
                    .text_size(px(10.))
                    .text_color(p.soft)
                    .font_weight(FontWeight::MEDIUM)
                    .child(title),
            );
            for choice in ExportFormat::ALL
                .into_iter()
                .filter(|format| format.group() == group)
            {
                if !self
                    .catalog
                    .as_ref()
                    .and_then(|catalog| catalog["formats"].as_array())
                    .is_some_and(|formats| formats.iter().any(|item| item["id"] == choice.id()))
                {
                    continue;
                }
                let reason = self.export_unavailable_reason(choice.id());
                let button =
                    Button::new(SharedString::from(format!("export-format-{}", choice.id())))
                        .ghost()
                        .w_full()
                        .h(px(if reason.is_some() { 58. } else { 36. }))
                        .px(px(10.))
                        .rounded(px(9.))
                        .disabled(reason.is_some() || self.export_prompt_busy)
                        .accessibility_label(choice.label(en))
                        .when(format == choice, |button| {
                            button.bg(p.selected).border_1().border_color(p.edge)
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .w_full()
                                .child(
                                    crate::ui_icons::icon(choice.icon()).text_color(
                                        if reason.is_some() { p.faint } else { p.soft },
                                    ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap(px(1.))
                                        .text_size(px(12.))
                                        .child(div().truncate().child(choice.label(en)))
                                        .when_some(reason, |row, reason| {
                                            row.child(
                                                div()
                                                    .text_size(px(10.))
                                                    .line_height(px(14.))
                                                    .text_color(p.soft)
                                                    .child(reason),
                                            )
                                        }),
                                )
                                .when(format == choice, |row| {
                                    row.child(crate::ui_icons::icon("check").size(px(14.)))
                                }),
                        )
                        .on_click(
                            cx.listener(move |view, _, _, cx| {
                                view.choose_export_format(choice, cx)
                            }),
                        );
                section = section.child(button);
            }
            sidebar = sidebar.child(section);
        }
        let mut preview = div()
            .id("export-preview")
            .test_support()
            .relative()
            .flex_1()
            .min_h(px(40.))
            .min_w_0()
            .my(px(18.))
            .overflow_hidden()
            .rounded(px(12.))
            .bg(p.ink.opacity(0.025));
        if let Some(error) = &self.export_preview.error {
            preview = preview.child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(12.))
                    .p(px(20.))
                    .text_size(px(12.))
                    .child(error.clone())
                    .when(
                        !self
                            .current_tile()
                            .is_some_and(|tile| tile.error.as_deref() == Some("__empty__")),
                        |body| {
                            body.child(
                                Button::new("retry-export-preview")
                                    .ghost()
                                    .shell_label(self.text("重试预览", "Retry preview"))
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.export_preview.clear(cx);
                                        if view
                                            .current_tile()
                                            .is_some_and(|tile| tile.error.is_some())
                                        {
                                            view.rebuild_preserving_view(cx);
                                        }
                                        cx.notify();
                                    })),
                            )
                        },
                    ),
            );
        } else if let Some(image) = &self.export_preview.image {
            preview = preview.child(crate::material::contained_preview(
                image.clone(),
                10.,
                self.transparent && format.transparency(),
            ));
        } else if self.export_preview.text.is_some() {
            let lines = self.export_preview.lines.clone();
            let line_width = (lines
                .iter()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(1) as f32
                * 7.)
                .max(1.);
            preview = preview.child(
                div()
                    .id("export-text-scroll")
                    .size_full()
                    .min_w_0()
                    .overflow_x_scroll()
                    .p(px(14.))
                    .child(
                        uniform_list("export-text-preview", lines.len(), move |range, _, _| {
                            range
                                .map(|index| {
                                    div()
                                        .h(px(19.))
                                        .line_height(px(19.))
                                        .text_size(px(11.))
                                        .font_family("monospace")
                                        .whitespace_nowrap()
                                        .child(lines[index].clone())
                                })
                                .collect::<Vec<_>>()
                        })
                        .h_full()
                        .w_full()
                        .min_w(px(line_width)),
                    ),
            );
        } else {
            preview = preview.child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.))
                    .text_color(p.soft)
                    .child(self.text("正在生成预览…", "Preparing preview…")),
            );
        }
        let mut properties = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(20.))
            .min_h(px(42.))
            .border_t_1()
            .border_color(p.edge)
            .pt(px(12.));
        if format.visual() {
            let sizes = self
                .catalog
                .as_ref()
                .and_then(|catalog| catalog["sizes"].as_array())
                .cloned()
                .unwrap_or_default();
            let choices = sizes
                .iter()
                .filter_map(|size| {
                    size["id"].as_str().map(|id| {
                        (
                            id.to_owned(),
                            self.export_size_label(id),
                            size["w"].as_u64().unwrap_or(0),
                            size["h"].as_u64().unwrap_or(0),
                        )
                    })
                })
                .collect::<Vec<_>>();
            let selected = self.export_size.clone();
            let entity = cx.entity().downgrade();
            let size_button = Button::new("export-size")
                .ghost()
                .h(px(30.))
                .px(px(10.))
                .rounded(px(7.))
                .border_1()
                .border_color(p.edge)
                .disabled(self.export_prompt_busy)
                .accessibility_label(self.text("画幅", "Canvas size"))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .text_size(px(12.))
                        .child(self.export_size_label(&self.export_size))
                        .child(crate::ui_icons::icon("down").size(px(12.))),
                )
                .dropdown_menu_with_anchor(Anchor::BottomRight, move |mut menu, _, _| {
                    for (id, label, w, h) in &choices {
                        let label = if *w > 0 {
                            format!("{label} · {w} × {h}")
                        } else {
                            label.clone()
                        };
                        let id = id.clone();
                        let entity = entity.clone();
                        menu =
                            menu.item(PopupMenuItem::new(label).checked(id == selected).on_click(
                                move |_, _, cx| {
                                    let _ = entity.update(cx, |view, cx| {
                                        view.export_size = id.clone();
                                        view.preferences.export_size = id.clone();
                                        view.notice = None;
                                        view.persist();
                                        cx.notify();
                                    });
                                },
                            ));
                    }
                    menu
                });
            properties = properties.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .text_size(px(12.))
                    .child(self.text("画幅", "Canvas size"))
                    .child(size_button),
            );
            if format.transparency() {
                properties = properties.child(
                    Button::new("transparent")
                        .ghost()
                        .h(px(30.))
                        .px(px(7.))
                        .rounded(px(7.))
                        .disabled(self.export_prompt_busy)
                        .accessibility_label(self.text("透明背景", "Transparent background"))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(9.))
                                .text_size(px(12.))
                                .child(self.text("透明背景", "Transparent background"))
                                .child(
                                    div()
                                        .relative()
                                        .w(px(27.))
                                        .h(px(16.))
                                        .rounded_full()
                                        .bg(if self.transparent {
                                            p.ink
                                        } else {
                                            p.ink.opacity(0.18)
                                        })
                                        .child(
                                            div()
                                                .absolute()
                                                .left(px(if self.transparent { 14. } else { 3. }))
                                                .top(px(3.))
                                                .size(px(10.))
                                                .rounded_full()
                                                .bg(if self.transparent { p.bg } else { p.soft }),
                                        ),
                                ),
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.transparent = !view.transparent;
                            view.notice = None;
                            cx.notify();
                        })),
                );
            }
            if format.fonts() {
                properties = properties.child(
                    Button::new("font-mode")
                        .ghost()
                        .h(px(30.))
                        .px(px(7.))
                        .rounded(px(7.))
                        .disabled(self.export_prompt_busy)
                        .accessibility_label(self.text("文字转路径", "Outline text"))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(7.))
                                .text_size(px(12.))
                                .child(crate::ui_icons::icon(if self.font_mode == "outline" {
                                    "check"
                                } else {
                                    "file"
                                }))
                                .child(self.text("文字转路径", "Outline text")),
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.font_mode = if view.font_mode == "outline" {
                                "embed"
                            } else {
                                "outline"
                            }
                            .into();
                            view.notice = None;
                            cx.notify();
                        })),
                );
            }
        }
        let action = if format.visual() {
            format!(
                "{} {}…",
                self.text("导出", "Export"),
                format.id().to_uppercase()
            )
        } else {
            self.text(
                match format {
                    ExportFormat::Md => "复制 CREDITS.md",
                    ExportFormat::Embed => "复制嵌入代码",
                    ExportFormat::Json => "复制 JSON",
                    _ => "复制文本",
                },
                match format {
                    ExportFormat::Md => "Copy CREDITS.md",
                    ExportFormat::Embed => "Copy embed code",
                    ExportFormat::Json => "Copy JSON",
                    _ => "Copy text",
                },
            )
            .to_owned()
        };
        let unavailable = if format.visual() {
            !self.can_export() || !self.export_history_writable()
        } else {
            self.export_preview.text.is_none()
        };
        let primary = Button::new("export-primary")
            .w(px(172.))
            .h(px(38.))
            .rounded(px(9.))
            .bg(p.ink)
            .text_color(p.bg)
            .icon(crate::ui_icons::icon(if format.visual() {
                "export"
            } else {
                "copy"
            }))
            .shell_label(action)
            .disabled(unavailable || self.export_prompt_busy)
            .on_click(cx.listener(move |view, _, window, cx| {
                if format.visual() {
                    view.export_artwork(&ExportArtwork, window, cx);
                } else if let Some(text) = &view.export_preview.text {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                    view.notice = Some(view.text("已复制", "Copied").into());
                    cx.notify();
                }
            }));
        let filename = if format.visual() {
            self.export_request()
                .ok()
                .map(|request| export_filename(&request))
                .unwrap_or_default()
        } else {
            format.label(en).into()
        };
        let mut detail = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .p(px(if self.viewport_size.0 < 1000. {
                18.
            } else {
                26.
            }))
            .child(
                div()
                    .id("export-title")
                    .test_support()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(format.label(en)),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.soft)
                            .child(format.description(en)),
                    ),
            )
            .child(preview);
        if format.visual() {
            detail = detail.child(properties);
        }
        detail = detail.child(
            div()
                .flex()
                .items_center()
                .gap(px(16.))
                .pt(px(16.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(11.))
                        .text_color(p.soft)
                        .truncate()
                        .child(if self.export_stage.is_some() {
                            format!(
                                "{} · {}",
                                self.text(
                                    if self.export_cancel_requested {
                                        "正在取消"
                                    } else {
                                        "正在导出"
                                    },
                                    if self.export_cancel_requested {
                                        "Canceling"
                                    } else {
                                        "Exporting"
                                    }
                                ),
                                self.export_label
                            )
                        } else {
                            self.notice.clone().unwrap_or(filename)
                        }),
                )
                .when(self.export_stage.is_some(), |footer| {
                    footer.child(
                        Button::new("cancel-export-workspace")
                            .ghost()
                            .w(px(28.))
                            .h(px(28.))
                            .p_0()
                            .rounded(px(8.))
                            .icon(crate::ui_icons::icon("close"))
                            .accessibility_label(self.text("取消导出", "Cancel export"))
                            .tooltip(self.text("取消导出", "Cancel export"))
                            .disabled(self.export_cancel_requested)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.cancel_export(&CancelExport, window, cx)
                            })),
                    )
                })
                .child(primary),
        );
        if let Some(error) = &self.error {
            detail = detail.child(div().mt(px(8.)).text_size(px(11.)).child(error.clone()));
        }
        let piece = self
            .selected_piece()
            .and_then(|piece| piece[if en { "en" } else { "name" }].as_str())
            .unwrap_or("");
        let project = self
            .raw
            .as_ref()
            .and_then(|raw| raw["project"]["name"].as_str())
            .unwrap_or("");
        div()
            .id("export-workspace")
            .test_support()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.ink)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .h(px(60.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .pl(px(if cfg!(target_os = "macos") { 88. } else { 24. }))
                    .pr(px(24.))
                    .border_b_1()
                    .border_color(p.edge)
                    .child(
                        Button::new("export-back")
                            .ghost()
                            .h(px(32.))
                            .px(px(8.))
                            .rounded(px(8.))
                            .icon(crate::ui_icons::icon("prev"))
                            .shell_label(self.text("返回作品", "Back to artwork"))
                            .on_click(
                                cx.listener(|view, _, _, cx| view.close_export_workspace(cx)),
                            ),
                    )
                    .child(div().text_color(p.faint).child("/"))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.text("导出作品", "Export artwork")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.))
                            .text_color(p.soft)
                            .child(format!("{project} · {piece}")),
                    ),
            )
            .child(div().flex_1().min_h_0().flex().child(sidebar).child(detail))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{ExportFormat, export_text};
    #[test]
    fn text_preview_and_copy_share_the_exact_markdown_document() {
        let text = export_text(ExportFormat::Md, b"catnap\n".to_vec()).unwrap();
        assert!(text.starts_with("# Credits\n\n<!-- creditia:start -->\n"));
        assert!(text.ends_with("catnap\n\n<!-- creditia:end -->\n"));
        assert_eq!(
            export_text(ExportFormat::Json, b"{\"project\":\"catnap\"}\n".to_vec()).unwrap(),
            "{\"project\":\"catnap\"}\n"
        );
    }
}
