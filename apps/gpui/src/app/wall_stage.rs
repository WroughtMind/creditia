use super::chrome::{analysis_fraction, telemetry_enabled};
use super::export::ExportFormat;
use super::{Creditia, DragState, OpenProject, Panel, Playback, Tile};
use crate::Language;
use crate::live_mark::MarkStatus;
use crate::material::{Palette, ShellLabel};
use crate::ui_model::{Rect, texture_width};
use gpui_kit::TestSupportExt as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Disableable};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::time::Duration;

impl Creditia {
    pub(super) fn render_stage(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visible = Rect {
            x: -50.,
            y: -80.,
            w: self.viewport_size.0 + 100.,
            h: self.viewport_size.1 + 160.,
        };
        let mut stage = div()
            .id("artwork-stage")
            .test_support()
            .relative()
            .size_full()
            .overflow_hidden()
            .on_scroll(cx.listener(|view, event, _, cx| view.wheel(event, cx)))
            .on_pinch(cx.listener(|view, event, _, cx| view.pinch(event, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, event: &MouseDownEvent, _, cx| {
                    if view.export_workspace_open() {
                        view.export_formats_open = false;
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    view.pointer = (event.position.x.into(), f32::from(event.position.y) - 60.);
                    if view.panel.is_some() {
                        cx.stop_propagation();
                        return;
                    }
                    view.roam_motion.stop();
                    view.drag = Some(DragState {
                        tile: None,
                        clicked_tile: None,
                        source: None,
                        start: view.pointer,
                        last: view.pointer,
                        moved: false,
                    });
                    view.hover = None;
                    cx.notify();
                }),
            );
        if self.focused {
            let palette = Palette::of(cx);
            stage = stage.child(crate::material::surface(
                self.backdrop_background
                    .clone()
                    .or_else(|| self.backdrop_image.clone()),
                palette.bg.opacity(0.72),
                0.,
                palette,
            ));
        }
        if self.loading {
            if self.activity_age() >= Duration::from_millis(300) {
                stage = stage.child(self.render_loading_stage(window, cx));
            }
        } else if self.raw.is_none() {
            stage = stage.child(self.render_empty_stage(window, cx));
        } else {
            for index in 0..self.tiles.len() {
                if (!self.focused || self.selected_tile == Some(index))
                    && self.screen_rect(index).intersects(visible)
                {
                    let artwork = self.artwork_element(index, cx);
                    stage = if self.focused {
                        stage.child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .w_full()
                                .h(px(self.artwork_viewport().h))
                                .overflow_hidden()
                                .child(artwork),
                        )
                    } else {
                        stage.child(artwork)
                    };
                }
            }
        }
        if self.hover.is_some() {
            stage = stage.child(self.hover_card(cx));
        }
        stage.into_any_element()
    }

    fn render_loading_stage(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .flex_col()
            .gap_4()
            .child(self.center_mark.render(
                MarkStatus {
                    progress: self.progress.as_ref().and_then(analysis_fraction),
                    busy: true,
                    error: false,
                },
                self.reduced_motion,
                64.,
                window,
                cx,
            ))
            .when(self.activity_age() >= Duration::from_secs(2), |view| {
                view.child(self.text("正在读取作品库…", "Loading project library…"))
            })
    }

    fn render_empty_stage(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let mut empty = div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .size_full()
            .child(
                div()
                    .text_3xl()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(self.center_mark.render(
                        MarkStatus {
                            progress: self.progress.as_ref().and_then(analysis_fraction),
                            busy: self.job_busy || self.discover_busy,
                            error: self.error.is_some(),
                        },
                        self.reduced_motion,
                        56.,
                        window,
                        cx,
                    ))
                    .child("/creditia"),
            )
            .child(div().text_color(cx.theme().muted_foreground).child(
                if self.selected.is_some() {
                    self.text(
                        "分析项目，生成它的作品。",
                        "Analyze this project to create its artwork.",
                    )
                } else {
                    self.text(
                        "代码背后的工具与模型，成为一面作品墙。",
                        "The tools and models behind your code, on a wall of artwork.",
                    )
                },
            ))
            .child(
                Button::new("discover-projects")
                    .ghost()
                    .shell_label(self.text("查找本机项目…", "Find local projects…"))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.open_panel(Panel::Projects, window, cx)
                    })),
            )
            .child(
                Button::new("first-project")
                    .shell_label(if self.selected.is_some() {
                        self.text("分析项目", "Analyze project")
                    } else {
                        self.text("添加项目…", "Add project…")
                    })
                    .on_click(cx.listener(|view, _, window, cx| {
                        if view.selected.is_some() {
                            view.start_job(None, cx)
                        } else {
                            view.open_project(&OpenProject, window, cx)
                        }
                    })),
            );
        if self.selected.is_none() {
            empty = empty.child(self.render_welcome_candidates(cx));
        }
        empty
    }

    fn render_welcome_candidates(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut choices = div()
            .id("welcome-candidates")
            .w(px(540.))
            .max_h(px((self.viewport_size.1 - 290.).max(100.)))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        if self.discover_busy {
            choices = choices.child(self.text("正在查找本机项目…", "Finding local projects…"));
        }
        for (index, project) in self.discovered.as_ref().into_iter().flatten().enumerate() {
            let path = project["path"].as_str().unwrap_or("").to_string();
            let picked = self.discovery_selected.contains(&path);
            let name = project["name"].as_str().unwrap_or(&path).to_string();
            choices = choices.child(
                Button::new(("welcome-project", index))
                    .ghost()
                    .shell_label(format!("{} {}", if picked { "✓" } else { "○" }, name))
                    .disabled(self.job_busy)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if let Some(at) = view.discovery_selected.iter().position(|p| p == &path) {
                            view.discovery_selected.remove(at);
                        } else {
                            view.discovery_selected.push(path.clone());
                        }
                        cx.notify();
                    })),
            );
        }
        if let Some(error) = &self.discovery_error {
            choices = choices
                .child(div().text_size(px(12.)).child(error.clone()))
                .child(
                    Button::new("welcome-retry")
                        .ghost()
                        .shell_label(self.text("重新查找", "Try again"))
                        .on_click(cx.listener(|view, _, _, cx| view.retry_discovery(cx))),
                );
        } else if self.discovered.as_ref().is_some_and(Vec::is_empty) {
            choices = choices.child(self.text(
                "没有找到含 AI 记录的项目，可以选择一个文件夹。",
                "No projects with AI records were found. Choose a folder to begin.",
            ));
        }
        if !self.discovery_selected.is_empty() {
            choices = choices.child(
                Button::new("welcome-analyze")
                    .shell_label(format!(
                        "{} ({})",
                        self.text("分析所选项目", "Analyze selected"),
                        self.discovery_selected.len()
                    ))
                    .disabled(self.job_busy)
                    .on_click(cx.listener(|view, _, window, cx| {
                        let paths = std::mem::take(&mut view.discovery_selected);
                        view.queue_projects(paths, cx);
                        view.open_panel(Panel::Projects, window, cx);
                    })),
            );
        }
        choices
    }

    fn artwork_element(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let selection = &self.tiles[index];
        let tile = if self.focused {
            self.focus_placeholder.as_ref().unwrap_or(selection)
        } else {
            selection
        };
        let r = self.screen_rect(index);
        let title = self
            .piece(&tile.spec.piece)
            .and_then(|p| {
                p[if self.preferences.language == Language::En {
                    "en"
                } else {
                    "name"
                }]
                .as_str()
            })
            .unwrap_or(&tile.spec.piece)
            .to_string();
        let (picture, framed_export) = self.render_artwork_picture(tile, index, r, cx);
        let mut card = div()
            .absolute()
            .left(px(r.x))
            .top(px(r.y + 60.))
            .w(px(r.w))
            .rounded(px(10.))
            .when(!framed_export, |card| {
                card.shadow(vec![
                    BoxShadow::new(px(0.), px(12.), rgba(0x00000014).into()).blur_radius(px(32.)),
                ])
            })
            .child(picture);
        if !self.focused {
            card = self.decorate_artwork_card(card, tile, index, r.w, title, cx);
        }
        card.into_any_element()
    }

    fn render_artwork_picture(
        &self,
        tile: &Tile,
        index: usize,
        r: Rect,
        cx: &mut Context<Self>,
    ) -> (impl IntoElement, bool) {
        let selected = self.selected_tile == Some(index);
        let dragging = self
            .drag
            .as_ref()
            .is_some_and(|drag| drag.moved && drag.tile == Some(index));
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let export_preview = selected && self.export_workspace_open();
        let animation = (selected && (self.focused || self.export_animation()))
            .then(|| self.playback.as_ref().and_then(Playback::displayed))
            .flatten();
        let image = tile.image.clone();
        let mut picture = div()
            .id(("artwork", index))
            .test_support()
            .relative()
            .w(px(r.w))
            .h(px(r.h))
            .overflow_hidden()
            .rounded(px(10.))
            .cursor_pointer()
            .when(dragging, |picture| picture.opacity(0.3))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event: &MouseDownEvent, _, cx| {
                    if view.export_workspace_open() {
                        view.export_formats_open = false;
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    view.pointer = (event.position.x.into(), f32::from(event.position.y) - 60.);
                    if view.panel.is_some() {
                        cx.stop_propagation();
                        return;
                    }
                    view.selected_tile = Some(index);
                    view.roam_motion.stop();
                    view.drag = Some(DragState {
                        tile: if view.focused || view.preferences.view == "roam" {
                            None
                        } else {
                            Some(index)
                        },
                        clicked_tile: Some(index),
                        source: Some(view.screen_rect(index)),
                        start: view.pointer,
                        last: view.pointer,
                        moved: false,
                    });
                    cx.stop_propagation();
                    cx.notify();
                }),
            );
        let export_image = if export_preview && !self.export_animation() {
            self.export_preview.displayed_image()
        } else {
            None
        };
        if let Some(image) = export_image {
            picture = picture.child(crate::material::contained_preview(
                image.clone(),
                10.,
                self.transparent
                    && ExportFormat::from_id(&self.export_format)
                        .is_some_and(ExportFormat::transparency),
            ));
        } else if let Some(frame) = animation {
            let bounds = if self.export_animation() {
                self.export_frame_rect()
                    .map(|rect| Rect {
                        x: rect.x - r.x,
                        y: rect.y - r.y,
                        ..rect
                    })
                    .unwrap_or(Rect {
                        x: 0.,
                        y: 0.,
                        w: r.w,
                        h: r.h,
                    })
            } else {
                Rect {
                    x: 0.,
                    y: 0.,
                    w: r.w,
                    h: r.h,
                }
            };
            for (part, (region, image)) in frame.tiles.iter().enumerate() {
                let region = region.unwrap_or(Rect {
                    x: 0.,
                    y: 0.,
                    w: frame.art.w,
                    h: frame.art.h,
                });
                let exact = frame.pixels == texture_width(bounds.w, self.display_scale)
                    && self
                        .animation_control
                        .as_ref()
                        .is_some_and(|control| control.epoch() == frame.epoch);
                let scale = frame.pixels as f32 / frame.art.w / self.display_scale.max(0.1);
                let actual = frame.regions.get(part);
                let (left, top, width, height) = if exact {
                    (
                        (region.x * scale * self.display_scale).round() / self.display_scale,
                        (region.y * scale * self.display_scale).round() / self.display_scale,
                        actual.and_then(|r| r["width"].as_u64()).unwrap_or(1) as f32
                            / self.display_scale,
                        actual.and_then(|r| r["height"].as_u64()).unwrap_or(1) as f32
                            / self.display_scale,
                    )
                } else {
                    (
                        region.x / frame.art.w * bounds.w,
                        region.y / frame.art.h * bounds.h,
                        region.w / frame.art.w * bounds.w,
                        region.h / frame.art.h * bounds.h,
                    )
                };
                picture = picture.child(crate::material::frame_preview(
                    image.clone(),
                    Rect {
                        x: left + bounds.x,
                        y: top + bounds.y,
                        w: width,
                        h: height,
                    },
                    crate::material::image_corners(region, frame.art.w, frame.art.h),
                    (part + 1 == frame.tiles.len()).then(|| frame.painted.clone()),
                    telemetry_enabled().then_some((frame.epoch, frame.index)),
                ));
            }
        } else if let Some(image) = image {
            if let (Some(region), Some(art)) = (tile.image_region, tile.art.as_deref()) {
                let exact =
                    tile.pixels == texture_width(r.w, self.display_scale) && tile.image_size.0 > 0;
                let scale = tile.pixels as f32 / art.w / self.display_scale;
                let (left, top, width, height) = if exact {
                    (
                        (region.x * scale * self.display_scale).round() / self.display_scale,
                        (region.y * scale * self.display_scale).round() / self.display_scale,
                        tile.image_size.0 as f32 / self.display_scale,
                        tile.image_size.1 as f32 / self.display_scale,
                    )
                } else {
                    (
                        region.x / art.w * r.w,
                        region.y / art.h * r.h,
                        region.w / art.w * r.w,
                        region.h / art.h * r.h,
                    )
                };
                picture = picture.child(crate::material::preview(
                    image,
                    Rect {
                        x: left,
                        y: top,
                        w: width,
                        h: height,
                    },
                    crate::material::image_corners(region, art.w, art.h),
                ));
            } else {
                picture = picture.child(crate::material::preview(
                    image,
                    Rect {
                        x: 0.,
                        y: 0.,
                        w: r.w,
                        h: if tile.image_size.0 > 0 {
                            r.w * tile.image_size.1 as f32 / tile.image_size.0 as f32
                        } else {
                            r.h
                        },
                    },
                    Corners::all(px(10.)),
                ));
            }
        } else {
            picture = picture
                .bg(cx.theme().muted)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(div().text_size(px(12.)).text_color(muted).child(
                    if tile.error.as_deref() == Some("__empty__") {
                        self.text("暂无相应数据", "No data for this piece")
                            .to_string()
                    } else if let Some(error) = &tile.error {
                        error.clone()
                    } else {
                        self.text("正在排版…", "Preparing artwork…").to_string()
                    },
                ))
                .when(
                    tile.error
                        .as_deref()
                        .is_some_and(|error| error != "__empty__"),
                    |picture| {
                        picture.child(
                            Button::new(("retry-artwork", index))
                                .ghost()
                                .shell_label(self.text("重试", "Retry"))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if let Some(tile) = view.tiles.get_mut(index) {
                                        tile.error = None;
                                        tile.loading = false;
                                    }
                                    cx.notify();
                                })),
                        )
                    },
                );
        }
        let framed_export = export_preview
            && self.export_size != "native"
            && ExportFormat::from_id(&self.export_format).is_some_and(|format| format.visual());
        let edge_rect = if framed_export {
            self.export_frame_rect().map(|rect| Rect {
                x: rect.x - r.x,
                y: rect.y - r.y,
                ..rect
            })
        } else {
            None
        }
        .unwrap_or(Rect {
            x: 0.,
            y: 0.,
            w: r.w,
            h: r.h,
        });
        picture = picture.child(
            div()
                .absolute()
                .left(px(edge_rect.x))
                .top(px(edge_rect.y))
                .w(px(edge_rect.w))
                .h(px(edge_rect.h))
                .rounded(px(10.))
                .border_1()
                .border_color(if self.hovered_tile == Some(index) {
                    border
                } else {
                    rgba(if cx.theme().is_dark() {
                        0xFFFFFF0F
                    } else {
                        0x00000012
                    })
                    .into()
                }),
        );
        (picture, framed_export)
    }

    fn decorate_artwork_card(
        &self,
        mut card: Div,
        tile: &Tile,
        index: usize,
        width: f32,
        title: String,
        cx: &mut Context<Self>,
    ) -> Div {
        let selected = self.selected_tile == Some(index);
        let palette = Palette::of(cx);
        let variant_id = tile.spec.variant.as_deref().unwrap_or("");
        let piece = self.piece(&tile.spec.piece);
        let variant = piece
            .and_then(|piece| piece["variants"].as_array())
            .and_then(|variants| variants.iter().find(|variant| variant["id"] == variant_id))
            .and_then(|variant| {
                variant[if self.preferences.language == Language::En {
                    "en"
                } else {
                    "name"
                }]
                .as_str()
            })
            .unwrap_or(variant_id)
            .to_string();
        let hovered = self.hovered_tile == Some(index) || (self.export_open && selected);
        let purpose = piece
            .and_then(|piece| {
                piece[if self.preferences.language == Language::En {
                    "qEn"
                } else {
                    "q"
                }]
                .as_str()
            })
            .unwrap_or("")
            .to_string();
        card = card.child(
            div()
                .absolute()
                .bottom(relative(1.))
                .mb(px(7.))
                .left_0()
                .max_w(px(width * 0.58))
                .text_size(px(12.))
                .text_color(if hovered { palette.ink } else { palette.soft })
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(div().truncate().child(title))
                        .child(div().text_color(palette.faint).truncate().child(variant)),
                )
                .when(hovered && !purpose.is_empty(), |caption| {
                    caption.child(
                        div()
                            .text_size(px(11.))
                            .text_color(palette.faint)
                            .truncate()
                            .child(purpose),
                    )
                }),
        );
        if hovered {
            card = card.child(
                div()
                    .absolute()
                    .inset(px(-9.))
                    .rounded(px(14.))
                    .border_1()
                    .border_dashed()
                    .border_color(palette.ink.opacity(0.55)),
            );
        }
        if hovered && !(self.export_workspace_open() && selected) {
            let mut bar = div()
                .absolute()
                .top(px(-24.))
                .right(px(-6.))
                .flex()
                .items_center()
                .gap(px(2.))
                .p(px(3.))
                .rounded_full()
                .shadow(palette.shadows())
                .child(crate::material::surface(
                    self.backdrop_image.clone(),
                    palette.glass,
                    16.,
                    palette,
                ));
            if piece
                .and_then(|piece| piece["variants"].as_array())
                .is_some_and(|variants| variants.len() > 1)
            {
                for (id, icon, direction, name) in [
                    (
                        "tile-previous-layout",
                        "prev",
                        -1,
                        self.text("上一种版式", "Previous layout"),
                    ),
                    (
                        "tile-next-layout",
                        "next",
                        1,
                        self.text("下一种版式", "Next layout"),
                    ),
                ] {
                    bar = bar.child(
                        Button::new((id, index))
                            .ghost()
                            .w(px(26.))
                            .h(px(26.))
                            .p_0()
                            .rounded_full()
                            .icon(crate::ui_icons::icon(icon))
                            .accessibility_label(name)
                            .tooltip(name)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.selected_tile = Some(index);
                                view.cycle_variant(direction, cx);
                            })),
                    );
                }
            }
            bar = bar.child(
                self.surface_anchors.wrap(
                    "card-export",
                    Button::new(("tile-export", index))
                        .ghost()
                        .w(px(26.))
                        .h(px(26.))
                        .p_0()
                        .rounded_full()
                        .icon(crate::ui_icons::icon("export"))
                        .accessibility_label(self.text("导出作品", "Export artwork"))
                        .tooltip(self.text("导出作品", "Export artwork"))
                        .disabled(!self.can_open_export_for(index))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.toggle_export(index, cx);
                        })),
                ),
            );
            card = card.child(bar);
        }
        card
    }

    pub(super) fn render_drag_ghost(&self) -> Option<AnyElement> {
        self.drag
            .as_ref()
            .filter(|drag| drag.moved)
            .and_then(|drag| {
                let tile = self.tiles.get(drag.tile?)?;
                let image = tile.image.clone()?;
                let art = tile.art.as_ref()?;
                if tile.image_region.is_some() {
                    return None;
                }
                let source = drag.source?;
                let width = art.w.min(240.);
                let height = width * art.h / art.w;
                let fx = ((drag.start.0 - source.x) / source.w).clamp(0., 1.);
                let fy = ((drag.start.1 - source.y) / source.h).clamp(0., 1.);
                Some(
                    div()
                        .absolute()
                        .left(px(self.pointer.0 - width * fx))
                        .top(px(60. + self.pointer.1 - height * fy))
                        .w(px(width))
                        .h(px(height))
                        .rounded_lg()
                        .overflow_hidden()
                        .opacity(0.92)
                        .child(img(image).size_full().object_fit(ObjectFit::Contain))
                        .into_any_element(),
                )
            })
    }

    pub(super) fn refresh_backdrop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pending) = self.backdrop_pending.take() {
            let mut ready = true;
            for image in std::iter::once(&pending.image)
                .chain(pending.background.as_ref())
                .chain(pending.top.as_ref())
            {
                ready &= crate::image_assets::track(image, window, cx).is_some();
            }
            if ready {
                if let Some(old) = self.backdrop_image.replace(pending.image) {
                    crate::image_assets::release(old, cx);
                }
                if let Some(old) =
                    std::mem::replace(&mut self.backdrop_background, pending.background)
                {
                    crate::image_assets::release(old, cx);
                }
                if let Some(old) = std::mem::replace(&mut self.backdrop_top, pending.top) {
                    crate::image_assets::release(old, cx);
                }
            } else {
                self.backdrop_pending = Some(pending);
                window.request_animation_frame();
                return;
            }
        }
        if self.backdrop_task.is_some() {
            return;
        }
        let size = window.viewport_size();
        let size = (f32::from(size.width), f32::from(size.height));
        let visible = Rect {
            x: 0.,
            y: 0.,
            w: size.0,
            h: size.1,
        };
        let mut layers = Vec::new();
        for (index, tile) in self.tiles.iter().enumerate() {
            let tile = if self.focused && self.selected_tile == Some(index) {
                self.focus_placeholder.as_ref().unwrap_or(tile)
            } else {
                tile
            };
            let rect = if self.focused {
                self.rects
                    .get(index)
                    .copied()
                    .map(|r| r.transformed(self.saved_viewport.unwrap_or_default()))
            } else {
                Some(self.screen_rect(index))
            };
            let Some(mut artwork) = rect else { continue };
            artwork.y += 60.;
            if !artwork.intersects(visible) {
                continue;
            }
            let Some(image) = tile
                .image
                .as_ref()
                .and_then(|image| crate::image_assets::track(image, window, cx))
            else {
                continue;
            };
            let rect = if let (Some(region), Some(art)) = (tile.image_region, tile.art.as_deref()) {
                Rect {
                    x: artwork.x + region.x / art.w * artwork.w,
                    y: artwork.y + region.y / art.h * artwork.h,
                    w: region.w / art.w * artwork.w,
                    h: region.h / art.h * artwork.h,
                }
            } else {
                artwork
            };
            layers.push(crate::material::Layer {
                image,
                rect,
                artwork,
            });
        }
        let mut foreground = Vec::new();
        if self.focused
            && (self.export_open
                || self.panel.is_some()
                || self.notice.is_some()
                || self.hover.is_some()
                || self.error.is_some())
            && let Some(index) = self.selected_tile
        {
            let mut artwork = self.screen_rect(index);
            artwork.y += 60.;
            if let Some(frame) = self.playback.as_ref().and_then(Playback::displayed) {
                for (region, image) in &frame.tiles {
                    let region = region.unwrap_or(Rect {
                        x: 0.,
                        y: 0.,
                        w: frame.art.w,
                        h: frame.art.h,
                    });
                    {
                        foreground.push(crate::material::Layer {
                            image: image.clone(),
                            artwork,
                            rect: Rect {
                                x: artwork.x + region.x / frame.art.w * artwork.w,
                                y: artwork.y + region.y / frame.art.h * artwork.h,
                                w: region.w / frame.art.w * artwork.w,
                                h: region.h / frame.art.h * artwork.h,
                            },
                        });
                    }
                }
            } else if let Some(tile) = self.tiles.get(index)
                && let Some(image) = tile
                    .image
                    .as_ref()
                    .and_then(|image| crate::image_assets::track(image, window, cx))
            {
                let rect =
                    if let (Some(region), Some(art)) = (tile.image_region, tile.art.as_deref()) {
                        Rect {
                            x: artwork.x + region.x / art.w * artwork.w,
                            y: artwork.y + region.y / art.h * artwork.h,
                            w: region.w / art.w * artwork.w,
                            h: region.h / art.h * artwork.h,
                        }
                    } else {
                        artwork
                    };
                foreground.push(crate::material::Layer {
                    image,
                    artwork,
                    rect,
                });
            }
        }
        let scene = crate::material::Scene {
            size,
            dark: cx.theme().is_dark(),
            focused: self.focused,
            header_fade: self.saved_viewport.unwrap_or(self.viewport).y < 0.
                || self.preferences.view == "roam",
            layers,
            foreground,
        };
        let key = scene.key();
        if self.backdrop_key == Some(key) {
            return;
        }
        let work = cx
            .background_executor()
            .spawn(async move { scene.render() });
        self.backdrop_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |view, cx| {
                view.backdrop_task = None;
                view.backdrop_key = Some(key);
                match result {
                    Ok(image) => {
                        view.backdrop_pending = Some(image);
                        view.backdrop_error = None;
                    }
                    Err(error) => {
                        view.backdrop_error = Some(error.clone());
                        view.error = Some(error);
                    }
                }
                cx.notify();
            });
        }));
    }
}
