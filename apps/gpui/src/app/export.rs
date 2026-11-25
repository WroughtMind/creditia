//! Export controls replace the bottom dock; the artwork stays on its canvas.
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
    pub(super) fn visual(self) -> bool {
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
    pub(super) fn displayed_image(&self) -> Option<&Arc<Image>> {
        self.image.as_ref().filter(|_| self.image_ready)
    }
    pub(super) fn diagnostic(&self) -> Value {
        json!({"busy":self.task.is_some(),"image_ready":self.image_ready,"text_ready":self.text.is_some(),"error":self.error,"revision":self.revision})
    }
}

pub(super) fn export_text(format: ExportFormat, bytes: Vec<u8>) -> Result<String, String> {
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
    pub(super) fn failed_export_job(&self) -> Option<&crate::export_jobs::Job> {
        self.export_queue
            .job(self.failed_export?)
            .filter(|job| job.state == crate::export_jobs::State::Failed)
    }

    pub(super) fn export_workspace_open(&self) -> bool {
        self.export_open
            && !self
                .drag
                .as_ref()
                .is_some_and(|drag| drag.moved && drag.tile.is_some())
    }
    pub(super) fn export_animation(&self) -> bool {
        self.export_workspace_open() && self.export_format == "gif" && !self.reduced_motion
    }
    pub(super) fn close_export_workspace(&mut self, cx: &mut Context<Self>) {
        if self.export_animation() {
            self.stop_playback(cx);
        }
        self.export_open = false;
        self.export_formats_open = false;
        self.export_preview.clear(cx);
        self.hover = None;
        cx.notify();
    }
    fn choose_export_format(&mut self, format: ExportFormat, cx: &mut Context<Self>) {
        if self.export_prompt_busy || self.export_unavailable_reason(format.id()).is_some() {
            return;
        }
        if self.export_format != format.id() {
            self.stop_playback(cx);
        }
        self.export_format = format.id().into();
        self.export_formats_open = false;
        self.preferences.export_format = self.export_format.clone();
        self.persist();
        cx.notify();
    }
    fn export_dimensions(&self) -> Option<(f32, f32)> {
        let art = self.current_tile()?.art.as_ref()?;
        Some(
            self.catalog
                .as_ref()
                .and_then(|catalog| catalog["sizes"].as_array())
                .and_then(|sizes| sizes.iter().find(|size| size["id"] == self.export_size))
                .filter(|size| size["w"].as_f64().unwrap_or(0.) > 0.)
                .map(|size| {
                    (
                        size["w"].as_f64().unwrap() as f32,
                        size["h"].as_f64().unwrap() as f32,
                    )
                })
                .unwrap_or((art.w, art.h)),
        )
    }
    pub(super) fn export_frame_rect(&self) -> Option<Rect> {
        let anchor = self.screen_rect(self.selected_tile?);
        let (w, h) = self.export_dimensions()?;
        let scale = (anchor.w / w).min(anchor.h / h);
        Some(Rect {
            x: anchor.x + (anchor.w - w * scale) / 2.,
            y: anchor.y + (anchor.h - h * scale) / 2.,
            w: w * scale,
            h: h * scale,
        })
    }
    pub(super) fn configure_playback_view(&self, control: &crate::animation::PlaybackControl) {
        if self.export_animation()
            && let Some(rect) = self.export_frame_rect()
        {
            control.set_artwork_view(
                rect,
                self.artwork_viewport(),
                self.viewport_size,
                self.display_scale,
            );
            return;
        }
        control.set_viewport(self.viewport, self.viewport_size, self.display_scale);
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
            let Some((w, _)) = self.export_dimensions() else {
                return;
            };
            self.export_frame_rect()
                .map(|rect| {
                    let scale = rect.w / w * self.display_scale;
                    if scale.is_nan() {
                        8.
                    } else {
                        scale.clamp(0.01, 8.)
                    }
                })
                .unwrap_or(1.)
        } else {
            1.
        };
        let key = format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            self.preview_generation,
            tile.identity,
            self.export_format,
            self.export_size,
            self.transparent,
            self.font_mode,
            self.preferences.language.code(),
            self.group(),
            self.preferences.named,
            scale.to_bits(),
            self.reduced_motion
        );
        if self.export_preview.key == key {
            return;
        }
        self.export_preview.clear(cx);
        self.export_preview.key = key.clone();
        if self.export_animation() {
            self.stop_playback(cx);
            self.play(false, cx);
            return;
        }
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
                        Ok(text) => view.export_preview.text = Some(text),
                        Err(error) => view.export_preview.error = Some(error),
                    },
                    Err(error) => view.export_preview.error = Some(error),
                }
                cx.notify();
            });
        }));
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
    pub(super) fn render_export_dock(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.export_workspace_open() {
            return div().into_any_element();
        }
        let p = Palette::of(cx);
        let en = self.preferences.language == Language::En;
        let format = ExportFormat::from_id(&self.export_format).unwrap_or(ExportFormat::Png);
        let mut dock = div()
            .id("export-dock")
            .test_support()
            .relative()
            .occlude()
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(46.))
            .p(px(5.))
            .rounded_full()
            .shadow(p.shadows())
            .border_1()
            .border_color(p.edge)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.drag = None;
                    cx.stop_propagation();
                }),
            )
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(crate::material::surface(
                self.backdrop_image.clone(),
                p.glass,
                23.,
                p,
            ))
            .child(
                Button::new("export-back")
                    .ghost()
                    .w(px(30.))
                    .h(px(34.))
                    .p_0()
                    .rounded_full()
                    .icon(crate::ui_icons::icon("close"))
                    .accessibility_label(self.text("收起导出", "Close export"))
                    .tooltip(self.text("收起导出", "Close export"))
                    .on_click(cx.listener(|view, _, _, cx| view.close_export_workspace(cx))),
            )
            .child(
                Button::new("export-format")
                    .ghost()
                    .h(px(34.))
                    .px(px(8.))
                    .rounded(px(10.))
                    .accessibility_label(self.text("选择格式", "Choose format"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(crate::ui_icons::icon(format.icon()))
                            .child(if format == ExportFormat::Md {
                                "MD".into()
                            } else {
                                format.id().to_uppercase()
                            })
                            .child(crate::ui_icons::icon("down").size(px(10.))),
                    )
                    .disabled(self.export_prompt_busy)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.export_formats_open = !view.export_formats_open;
                        cx.notify();
                    })),
            );
        if format.visual() {
            let choices = self
                .catalog
                .as_ref()
                .and_then(|catalog| catalog["sizes"].as_array())
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|size| {
                    size["id"]
                        .as_str()
                        .map(|id| (id.to_owned(), self.export_size_label(id)))
                })
                .collect::<Vec<_>>();
            let selected = self.export_size.clone();
            let entity = cx.entity().downgrade();
            dock = dock.child(
                Button::new("export-size")
                    .ghost()
                    .h(px(34.))
                    .px(px(8.))
                    .rounded(px(10.))
                    .disabled(self.export_prompt_busy)
                    .accessibility_label(self.text("画幅", "Canvas size"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7.))
                            .text_size(px(12.))
                            .child(self.export_size_label(&self.export_size))
                            .child(crate::ui_icons::icon("down").size(px(10.))),
                    )
                    .dropdown_menu_with_anchor(Anchor::BottomRight, move |mut menu, _, _| {
                        for (id, label) in &choices {
                            let id = id.clone();
                            let entity = entity.clone();
                            menu = menu.item(
                                PopupMenuItem::new(label.clone())
                                    .checked(id == selected)
                                    .on_click(move |_, _, cx| {
                                        let _ = entity.update(cx, |view, cx| {
                                            view.export_size = id.clone();
                                            view.preferences.export_size = id.clone();
                                            view.persist();
                                            cx.notify();
                                        });
                                    }),
                            );
                        }
                        menu
                    }),
            );
            if format.transparency() || format.fonts() {
                dock = dock.child(div().w(px(1.)).h(px(19.)).mx(px(3.)).bg(p.edge));
            }
            if format.transparency() {
                dock = dock.child(
                    Button::new("transparent")
                        .ghost()
                        .h(px(34.))
                        .px(px(8.))
                        .rounded(px(10.))
                        .when(self.transparent, |b| b.bg(p.selected))
                        .toggled(self.transparent)
                        .disabled(self.export_prompt_busy)
                        .icon(crate::ui_icons::icon(if self.transparent {
                            "check"
                        } else {
                            "transparent"
                        }))
                        .shell_label(self.text("透明", "Transparent"))
                        .accessibility_label(self.text("透明背景", "Transparent background"))
                        .tooltip(self.text("透明背景", "Transparent background"))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.transparent = !view.transparent;
                            cx.notify();
                        })),
                );
            }
            if format.fonts() {
                let editable = self.font_mode == "embed";
                dock = dock.child(
                    Button::new("editable-text")
                        .ghost()
                        .h(px(34.))
                        .px(px(8.))
                        .rounded(px(10.))
                        .when(editable, |b| b.bg(p.selected))
                        .toggled(editable)
                        .disabled(self.export_prompt_busy)
                        .icon(crate::ui_icons::icon(if editable {
                            "check"
                        } else {
                            "editable-text"
                        }))
                        .shell_label(self.text("可编辑文字", "Editable text"))
                        .accessibility_label(self.text("保留可编辑文字", "Keep editable text"))
                        .tooltip(self.text("保留可编辑文字", "Keep editable text"))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.font_mode = if view.font_mode == "outline" {
                                "embed"
                            } else {
                                "outline"
                            }
                            .into();
                            cx.notify();
                        })),
                );
            }
        }
        if format == ExportFormat::Gif {
            let playing = self.playback.as_ref().is_some_and(|play| play.playing);
            let ended = self.playback.as_ref().is_some_and(|play| play.ended);
            let label = if self.animation_busy {
                self.text("准备播放…", "Preparing playback…")
            } else if playing {
                self.text("暂停", "Pause")
            } else if ended {
                self.text("从头重播", "Replay from start")
            } else {
                self.text("继续播放", "Continue playback")
            };
            dock = dock.child(
                Button::new("export-play")
                    .ghost()
                    .w(px(34.))
                    .h(px(34.))
                    .p_0()
                    .rounded(px(10.))
                    .icon(crate::ui_icons::icon(if playing {
                        "pause"
                    } else if ended {
                        "replay"
                    } else {
                        "play"
                    }))
                    .accessibility_label(label)
                    .tooltip(if self.reduced_motion {
                        self.text("已减少动态效果", "Reduced motion is enabled")
                    } else {
                        label
                    })
                    .disabled(self.animation_busy || self.reduced_motion)
                    .on_click(cx.listener(|view, _, _, cx| view.play(false, cx))),
            );
        }
        if !format.visual() {
            dock = dock.child(
                Button::new("export-copy")
                    .ghost()
                    .w(px(34.))
                    .h(px(34.))
                    .p_0()
                    .rounded(px(10.))
                    .icon(crate::ui_icons::icon("copy"))
                    .accessibility_label(self.text("复制", "Copy"))
                    .tooltip(self.text("复制", "Copy"))
                    .disabled(self.export_preview.text.is_none() || self.export_prompt_busy)
                    .on_click(cx.listener(|view, _, _, cx| {
                        if let Some(text) = &view.export_preview.text {
                            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                            view.show_success(view.text("已复制", "Copied").into(), cx);
                        }
                    })),
            );
        }
        let unavailable = !self.can_export() || !self.export_history_writable();
        dock = dock.child(
            Button::new("export-primary")
                .h(px(34.))
                .px(px(14.))
                .rounded_full()
                .bg(p.ink)
                .text_color(p.bg)
                .icon(crate::ui_icons::icon("export"))
                .shell_label(self.text("导出", "Export"))
                .disabled(unavailable || self.export_prompt_busy)
                .on_click(cx.listener(|view, _, window, cx| {
                    view.export_artwork(&ExportArtwork, window, cx);
                })),
        );
        let mut stack = div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(18.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(10.));
        if self.export_formats_open {
            let mut grid = div()
                .id("export-formats")
                .relative()
                .occlude()
                .flex()
                .flex_col()
                .gap(px(4.))
                .p(px(8.))
                .rounded(px(18.))
                .shadow(p.shadows())
                .border_1()
                .border_color(p.edge)
                .child(crate::material::surface(
                    self.backdrop_image.clone(),
                    p.glass,
                    18.,
                    p,
                ))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation());
            for choices in ExportFormat::ALL.chunks(3) {
                let mut row = div().flex().gap(px(4.));
                for &choice in choices {
                    let reason = self.export_unavailable_reason(choice.id());
                    row = row.child(
                        Button::new(SharedString::from(format!("export-format-{}", choice.id())))
                            .ghost()
                            .w(px(76.))
                            .h(px(56.))
                            .p_0()
                            .rounded(px(10.))
                            .when(format == choice, |b| b.bg(p.selected))
                            .disabled(reason.is_some() || self.export_prompt_busy)
                            .accessibility_label(choice.label(en))
                            .tooltip(reason.unwrap_or(choice.label(en)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap(px(4.))
                                    .text_size(px(11.))
                                    .child(crate::ui_icons::icon(choice.icon()))
                                    .child(if choice == ExportFormat::Md {
                                        "MD".into()
                                    } else {
                                        choice.id().to_uppercase()
                                    }),
                            )
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.choose_export_format(choice, cx)
                            })),
                    );
                }
                grid = grid.child(row);
            }
            stack = stack.child(grid);
        }
        if let Some(error) = &self.export_preview.error {
            stack = stack.child(
                self.feedback_surface(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().min_w_0().flex_1().truncate().child(error.clone()))
                        .child(
                            Button::new("retry-export-preview")
                                .ghost()
                                .icon(crate::ui_icons::icon("replay"))
                                .accessibility_label(self.text("重试预览", "Retry preview"))
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.export_preview.clear(cx);
                                    cx.notify();
                                })),
                        )
                        .into_any_element(),
                    cx,
                ),
            );
        }
        div()
            .absolute()
            .inset_0()
            .child(stack.children(self.export_feedback(cx)).child(dock))
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
