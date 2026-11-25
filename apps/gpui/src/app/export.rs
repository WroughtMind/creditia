//! Export controls replace the bottom dock; the artwork stays on its canvas.

use super::{CancelExport, Creditia, ExportArtwork, ExportStage, Panel};
use crate::material::{Palette, ShellLabel};
use crate::ui_model::Rect;
use crate::{ExportOptions, Language, export_filename, export_with_preferences, renderer};
use creditia_desktop_service as service;
use gpui_kit::TestSupportExt as _;
use gpui_kit::component::Disableable;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde_json::{Value, json};
use std::sync::Arc;

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

impl Creditia {
    pub(super) fn can_export(&self) -> bool {
        self.export_unavailable_reason(&self.export_format)
            .is_none()
            && !self.export_prompt_busy
    }

    pub(super) fn export_unavailable_reason(&self, format: &str) -> Option<&'static str> {
        let Some(tile) = self.current_tile().filter(|_| self.raw.is_some()) else {
            return Some(self.text("请先选择作品", "Select an artwork first"));
        };
        if format == "gif"
            && self
                .selected_piece()
                .is_none_or(|piece| piece["animated"] != true)
        {
            return Some(self.text("静态作品不支持 GIF", "Static artwork does not support GIF"));
        }
        if !["txt", "md", "json", "embed"].contains(&format) {
            if tile.error.is_some() {
                return Some(self.text("作品预览不可用", "Artwork preview is unavailable"));
            }
            if tile.art.is_none() {
                return Some(self.text("作品预览准备中", "Preparing artwork preview"));
            }
        }
        None
    }

    pub(super) fn can_open_export_for(&self, index: usize) -> bool {
        self.raw.is_some() && self.tiles.get(index).is_some() && !self.export_prompt_busy
    }

    pub(super) fn toggle_export(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.can_open_export_for(index) {
            return;
        }
        let close = self.export_open && self.selected_tile == Some(index);
        if close {
            self.close_export_workspace(cx);
            return;
        }
        if self.selected_tile != Some(index) {
            self.stop_playback(cx);
        }
        self.selected_tile = Some(index);
        self.roam_motion.stop();
        self.export_open = true;
        {
            if self.playback.as_ref().is_some_and(|play| play.playing) {
                self.play(false, cx);
            }
            self.export_formats_open = false;
            if self
                .export_unavailable_reason(&self.export_format)
                .is_some()
            {
                self.export_format =
                    if self.current_tile().is_some_and(|tile| tile.error.is_some()) {
                        "md"
                    } else {
                        "png"
                    }
                    .into();
            }
            if !self
                .catalog
                .as_ref()
                .and_then(|c| c["sizes"].as_array())
                .is_some_and(|sizes| sizes.iter().any(|size| size["id"] == self.export_size))
            {
                self.export_size = "native".into();
            }
            self.notice = None;
        }
        self.hover = None;
        cx.notify();
    }

    pub(super) fn export_history_writable(&self) -> bool {
        self.fixture
            || (self.export_history_error.is_none()
                && self
                    .export_journal
                    .as_ref()
                    .is_some_and(|journal| !journal.blocked()))
    }

    fn export_request(&self) -> Result<Value, String> {
        self.export_request_format(&self.export_format)
    }

    fn export_request_format(&self, format: &str) -> Result<Value, String> {
        let raw = self.raw.as_ref().ok_or("No archive")?;
        let catalog = self.catalog.as_ref().ok_or("No artwork catalog")?;
        let tile = self.current_tile().ok_or("No artwork selected")?;
        let pieces = self
            .style()
            .and_then(|s| s["pieces"].as_array())
            .ok_or("No pieces")?;
        let piece_index = pieces
            .iter()
            .position(|p| p["id"] == tile.spec.piece)
            .ok_or("No piece")?;
        let variant_index = pieces[piece_index]["variants"]
            .as_array()
            .and_then(|vs| {
                vs.iter()
                    .position(|v| v["id"].as_str() == tile.spec.variant.as_deref())
            })
            .unwrap_or(0);
        let mut snapshot = export_with_preferences(
            raw,
            catalog,
            (self.style_index, piece_index, variant_index),
            &self.preferences,
            ExportOptions {
                format,
                size: &self.export_size,
                transparent: self.transparent
                    && ExportFormat::from_id(format).is_some_and(ExportFormat::transparency),
                font_mode: &self.font_mode,
            },
        )?;
        if !self.all_archives.is_empty() {
            snapshot["projects"] = json!(self.all_archives.as_ref());
        }
        if let Some(blame) = &self.blame {
            snapshot["blame"] = blame.clone();
        }
        if let Some(sign) = self
            .details
            .as_ref()
            .and_then(|d| d["config"]["display"]["sign"].as_str())
        {
            snapshot["sign"] = json!(sign);
        }
        if let Some(person) = &self.profile_person {
            snapshot["person"] = json!(person);
        }
        Ok(snapshot)
    }

    pub(super) fn load_export_history(
        &mut self,
        result: Result<Value, service::Fail>,
        cx: &mut Context<Self>,
    ) {
        if self.fixture {
            return;
        }
        let snapshot = match result {
            Ok(value) => value,
            Err(error) => {
                self.export_history_error = Some(error.to_string());
                self.error = Some(error.to_string());
                return;
            }
        };
        match crate::export_jobs::Queue::from_snapshot(snapshot["queue"].clone()) {
            Ok(queue) => self.export_queue = queue,
            Err(error) => {
                self.export_history_error = Some(error.clone());
                self.error = Some(error);
                return;
            }
        }
        self.export_history_error = None;
        let (tx, rx) = async_channel::bounded(1);
        self.export_journal = Some(crate::export_jobs::Journal::new(
            snapshot["revision"].as_u64().unwrap_or(0),
            Arc::new(move || {
                let _ = tx.try_send(());
            }),
        ));
        self.export_journal_task = Some(cx.spawn(async move |this, cx| {
            while rx.recv().await.is_ok() {
                if this
                    .update(cx, |view, cx| {
                        if let Some(error) = view
                            .export_journal
                            .as_ref()
                            .and_then(|journal| journal.take_error())
                        {
                            view.export_history_error = Some(error.clone());
                            view.error = Some(error);
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    pub(super) fn reload_export_history(&mut self, cx: &mut Context<Self>) {
        if self.fixture
            || self
                .export_queue
                .jobs()
                .iter()
                .any(|job| job.state.pending())
        {
            return;
        }
        self.report_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async { service::load_export_jobs() })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.load_export_history(result, cx);
                cx.notify();
            });
        }));
    }

    pub(super) fn set_collection_mode(&mut self, mode: &str, cx: &mut Context<Self>) {
        let on = mode != "off";
        let always = mode == "always";
        let supported = self.collect["agent"]["supported"]
            .as_bool()
            .unwrap_or(false);
        self.operation(
            move || {
                service::set_collect(on)?;
                if supported {
                    service::set_collect_agent(always)?;
                }
                Ok(service::collect_info())
            },
            cx,
        );
    }

    fn persist_exports(&self) {
        if let Some(journal) = &self.export_journal {
            journal.submit(self.export_queue.clone());
        }
    }

    fn export_artwork(&mut self, _: &ExportArtwork, _: &mut Window, cx: &mut Context<Self>) {
        if !self.can_export() || !self.export_history_writable() {
            return;
        }
        let snapshot = match self.export_request() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        let directory = match service::default_export_dir() {
            Ok(directory) => directory,
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let filename = export_filename(&snapshot);
        self.export_formats_open = false;
        let prompt = cx.prompt_for_new_path(&directory, Some(&filename));
        self.export_prompt_busy = true;
        self.export_dialog_task = Some(cx.spawn(async move |this, cx| {
            let choice = prompt.await;
            let _ = this.update(cx, |view, cx| {
                view.export_prompt_busy = false;
                match choice {
                    Ok(Ok(Some(_))) if !view.export_history_writable() => {
                        view.panel = Some(Panel::Exports);
                    }
                    Ok(Ok(Some(path))) => match view.export_queue.enqueue(snapshot, path, filename)
                    {
                        Ok(_) => {
                            view.failed_export = None;
                            view.persist_exports();
                            view.pump_exports(cx);
                        }
                        Err(error) => view.error = Some(error),
                    },
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => view.error = Some(error.to_string()),
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn pump_exports(&mut self, cx: &mut Context<Self>) {
        if self.export_active.is_some() {
            return;
        }
        let Some(job) = self.export_queue.start_next() else {
            return;
        };
        let id = job.id;
        let attempt = job.attempt;
        self.export_active = Some((id, attempt));
        self.export_stage = Some(ExportStage::Rendering);
        self.export_cancel_requested = false;
        let cancellation = renderer::Cancellation::default();
        self.export_cancellation = Some(cancellation.clone());
        self.persist_exports();
        self.export_task = Some(cx.spawn(async move |this, cx| {
            let render_cancel = cancellation.clone();
            let output_path = job.path.clone();
            let result = cx
                .background_spawn(async move {
                    let output = renderer::request(job.renderer_request(), render_cancel)?;
                    let bytes = renderer::decode_export(&output, &job.format)?;
                    if job.format == "md" {
                        export_text(ExportFormat::Md, bytes).map(String::into_bytes)
                    } else {
                        Ok(bytes)
                    }
                })
                .await;
            let result = match result {
                Ok(bytes) => {
                    if this
                        .update(cx, |view, cx| {
                            if view.export_active != Some((id, attempt)) {
                                return;
                            }
                            view.export_queue.mark_writing(id, attempt);
                            view.export_stage = Some(ExportStage::Writing);
                            view.persist_exports();
                            cx.notify();
                        })
                        .is_err()
                    {
                        cancellation.cancel();
                        return;
                    }
                    cx.background_spawn(async move {
                        renderer::write_export(&output_path, &bytes, cancellation)
                    })
                    .await
                }
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |view, cx| {
                if view.export_active != Some((id, attempt)) {
                    return;
                }
                let cancelled = view.export_cancel_requested && result.is_err();
                if cancelled {
                    view.export_queue.finish_cancelled(id, attempt);
                    view.show_success(view.text("已取消导出", "Export cancelled").into(), cx);
                } else {
                    match &result {
                        Ok(()) => view.show_success(view.text("已保存", "Saved").into(), cx),
                        Err(_) => view.failed_export = Some(id),
                    }
                    view.export_queue.finish(id, attempt, result);
                }
                view.export_active = None;
                view.export_stage = None;
                view.export_cancellation = None;
                view.export_cancel_requested = false;
                view.persist_exports();
                view.pump_exports(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn retry_export_job(&mut self, id: u64, cx: &mut Context<Self>) {
        if !self.export_history_writable() {
            return;
        }
        match self.export_queue.retry(id) {
            Ok(()) => {
                if self.failed_export == Some(id) {
                    self.failed_export = None;
                }
                self.persist_exports();
                self.pump_exports(cx);
            }
            Err(error) => self.error = Some(error),
        }
        cx.notify();
    }

    fn cancel_export_job(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.export_active.is_some_and(|active| active.0 == id) {
            if let Some(cancellation) = &self.export_cancellation {
                cancellation.cancel();
                self.export_cancel_requested = true;
            }
        } else {
            self.export_queue.cancel_queued(id);
            self.persist_exports();
        }
        cx.notify();
    }

    pub(super) fn cancel_export(
        &mut self,
        _: &CancelExport,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(cancellation) = &self.export_cancellation {
            cancellation.cancel();
            self.export_cancel_requested = true;
            cx.notify();
        }
    }

    pub(super) fn render_export_history(
        &self,
        mut body: Div,
        muted: Hsla,
        cx: &mut Context<Self>,
    ) -> Div {
        if let Some(error) = &self.export_history_error {
            body = body
                .child(div().text_size(px(12.)).child(error.clone()))
                .child(
                    Button::new("reload-export-history")
                        .shell_label(self.text("重新读取导出历史", "Reload export history"))
                        .disabled(
                            self.export_queue
                                .jobs()
                                .iter()
                                .any(|job| job.state.pending()),
                        )
                        .on_click(cx.listener(|view, _, _, cx| view.reload_export_history(cx))),
                );
        }
        if self.export_queue.jobs().is_empty() {
            body = body.child(self.text("尚无导出记录", "No export history yet"));
        }
        for (index, job) in self.export_queue.jobs().iter().enumerate().rev() {
            let id = job.id;
            let state = match job.state {
                crate::export_jobs::State::Queued => self.text("等待", "Queued"),
                crate::export_jobs::State::Rendering => self.text("正在导出", "Rendering"),
                crate::export_jobs::State::Writing => self.text("正在保存", "Saving"),
                crate::export_jobs::State::Saved => self.text("已保存", "Saved"),
                crate::export_jobs::State::Failed => self.text("失败", "Failed"),
                crate::export_jobs::State::Cancelled => self.text("已取消", "Cancelled"),
                crate::export_jobs::State::Interrupted => self.text("已中断", "Interrupted"),
            };
            let mut row = div().flex().flex_col().gap_1().child(
                div()
                    .text_size(px(12.))
                    .child(format!("{} · {}", job.label, state)),
            );
            if let Some(error) = &job.error {
                row = row.child(
                    div()
                        .text_size(px(11.))
                        .text_color(muted)
                        .child(error.clone()),
                );
            }
            let mut actions = div().flex().gap_1();
            if job.state == crate::export_jobs::State::Saved {
                let path = job.path.clone();
                actions = actions.child(
                    Button::new(("export-reveal", index))
                        .ghost()
                        .shell_label(self.text("打开所在位置", "Reveal file"))
                        .on_click(cx.listener(move |_, _, _, cx| cx.reveal_path(&path))),
                );
            }
            if matches!(
                job.state,
                crate::export_jobs::State::Failed
                    | crate::export_jobs::State::Cancelled
                    | crate::export_jobs::State::Interrupted
            ) {
                actions = actions.child(
                    Button::new(("export-retry", index))
                        .ghost()
                        .shell_label(self.text("重试", "Retry"))
                        .on_click(cx.listener(move |view, _, _, cx| view.retry_export_job(id, cx))),
                );
            }
            if job.state.pending() {
                actions = actions.child(
                    Button::new(("export-cancel", index))
                        .ghost()
                        .shell_label(self.text("取消", "Cancel"))
                        .on_click(
                            cx.listener(move |view, _, _, cx| view.cancel_export_job(id, cx)),
                        ),
                );
            } else {
                actions = actions.child(
                    Button::new(("export-remove", index))
                        .ghost()
                        .shell_label(self.text("移除记录", "Remove record"))
                        .disabled(!self.export_history_writable())
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.export_queue.remove(id);
                            view.persist_exports();
                            cx.notify();
                        })),
                );
            }
            body = body.child(row.child(actions));
        }
        body
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
