//! The original settings information structure, using native controls.
use super::*;

fn heading(text: &'static str, p: Palette) -> Div {
    div()
        .mt(px(18.))
        .mb(px(4.))
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.faint)
        .child(text)
}
fn row(label: &'static str, control: impl IntoElement, p: Palette) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .py(px(9.))
        .border_b_1()
        .border_color(p.edge)
        .child(div().min_w(px(112.)).text_color(p.soft).child(label))
        .child(div().flex_1().min_w_0())
        .child(control)
}
pub(super) fn switch(id: impl Into<ElementId>, on: bool, p: Palette) -> Button {
    Button::new(id)
        .ghost()
        .w(px(32.))
        .h(px(20.))
        .p_0()
        .rounded_full()
        .bg(if on { p.ink } else { p.ink.opacity(0.18) })
        .child(
            div()
                .absolute()
                .top(px(3.))
                .left(px(if on { 15. } else { 3. }))
                .w(px(14.))
                .h(px(14.))
                .rounded_full()
                .bg(if on { p.bg } else { rgb(0xFFFFFF).into() }),
        )
}

impl Creditia {
    pub(super) fn feedback_surface(
        &self,
        content: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = Palette::of(cx);
        div()
            .relative()
            .occlude()
            .max_w(px((self.viewport_size.0 - 36.).max(1.)))
            .min_w_0()
            .rounded_full()
            .px(px(16.))
            .py(px(9.))
            .text_size(px(13.))
            .text_color(p.ink)
            .shadow(p.shadows())
            .child(crate::material::surface(
                self.backdrop_image.clone(),
                p.solid,
                20.,
                p,
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.drag = None;
                    cx.stop_propagation();
                }),
            )
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(content)
            .into_any_element()
    }

    pub(super) fn general_feedback(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        if let Some(error) = &self.error {
            let history_error = self.export_history_error.as_ref() == Some(error);
            let pending = self
                .export_queue
                .jobs()
                .iter()
                .any(|job| job.state.pending());
            rows.push(
                self.feedback_surface(
                    div()
                        .id("feedback-error")
                        .test_support()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .min_w_0()
                        .child(div().flex_1().min_w_0().truncate().child(error.clone()))
                        .child(
                            Button::new("retry")
                                .ghost()
                                .shell_label(if history_error {
                                    self.text("重新读取导出记录", "Reload export history")
                                } else {
                                    self.text("重试", "Retry")
                                })
                                .disabled(history_error && pending)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.error = None;
                                    if history_error {
                                        view.reload_export_history(cx);
                                    } else {
                                        view.refresh(cx);
                                    }
                                })),
                        )
                        .into_any_element(),
                    cx,
                ),
            );
        }
        if !self.job_busy
            && self.automatic_job.is_none()
            && matches!(
                self.pending_analysis["state"].as_str(),
                Some("interrupted" | "failed" | "cancelled")
            )
        {
            rows.push(
                self.feedback_surface(
                    div()
                        .id("analysis-recovery")
                        .test_support()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(self.text("上次任务尚未完成", "The previous task did not finish"))
                        .child(
                            Button::new("resume-analysis")
                                .ghost()
                                .shell_label(self.text("继续分析", "Resume analysis"))
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.start_job_mode(None, true, cx)
                                })),
                        )
                        .into_any_element(),
                    cx,
                ),
            );
        }
        if let Some(notice) = self
            .notice
            .as_ref()
            .filter(|notice| self.error.as_ref() != Some(notice))
        {
            rows.push(
                self.feedback_surface(
                    div()
                        .id("notice")
                        .test_support()
                        .min_w_0()
                        .child(notice.clone())
                        .into_any_element(),
                    cx,
                ),
            );
        }
        if let Some(message) = &self.success_notice {
            rows.push(
                self.feedback_surface(
                    div()
                        .id("success-notice")
                        .test_support()
                        .child(message.clone())
                        .into_any_element(),
                    cx,
                ),
            );
        }
        if self.animation_busy && self.error.is_none() {
            rows.push(
                self.feedback_surface(
                    div()
                        .child(self.text("正在准备动画…", "Preparing animation…"))
                        .into_any_element(),
                    cx,
                ),
            );
        }
        rows
    }

    pub(super) fn export_feedback(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let mut rows = Vec::new();
        let project_name = self
            .raw
            .as_ref()
            .and_then(|raw| raw["project"]["name"].as_str());
        if let Some(stage) = self.export_stage {
            let other_project = self
                .export_active
                .and_then(|(id, _)| self.export_queue.job(id))
                .and_then(|job| job.request.as_ref())
                .and_then(|request| request.project_name())
                .filter(|name| Some(*name) != project_name)
                .map(str::to_owned);
            let label = match stage {
                ExportStage::Rendering => self.text("正在导出…", "Exporting…"),
                ExportStage::Writing => self.text("正在保存…", "Saving…"),
            };
            rows.push(
                self.feedback_surface(
                    div()
                        .id("export-progress")
                        .test_support()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .children(other_project)
                        .child(label)
                        .child(
                            Button::new("cancel-export")
                                .ghost()
                                .shell_label(self.text("取消", "Cancel"))
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.cancel_export(&CancelExport, window, cx);
                                })),
                        )
                        .into_any_element(),
                    cx,
                ),
            );
        }
        if let Some(job) = self.failed_export_job() {
            let id = job.id;
            let other_project = job
                .request
                .as_ref()
                .and_then(|request| request.project_name())
                .filter(|name| Some(*name) != project_name)
                .map(str::to_owned);
            rows.push(
                self.feedback_surface(
                    div()
                        .id("export-failure")
                        .test_support()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .min_w_0()
                        .children(other_project)
                        .child(self.text("导出失败", "Export failed"))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(job.error.clone().unwrap_or_default()),
                        )
                        .when(self.export_history_writable(), |row| {
                            row.child(
                                Button::new("retry-failed-export")
                                    .ghost()
                                    .shell_label(self.text("重试导出", "Retry export"))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.retry_export_job(id, cx)
                                    })),
                            )
                        })
                        .into_any_element(),
                    cx,
                ),
            );
        }
        (!rows.is_empty()).then(|| {
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .children(rows)
                .into_any_element()
        })
    }

    pub(super) fn analysis_progress(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::of(cx);
        div()
            .id("analysis-progress")
            .test_support()
            .flex()
            .items_center()
            .gap(px(8.))
            .min_w_0()
            .child(
                div()
                    .max_w(px(300.))
                    .text_size(px(12.))
                    .text_color(p.soft)
                    .truncate()
                    .child(
                        self.progress
                            .as_ref()
                            .map(|progress| progress_text(progress, &self.preferences.language))
                            .unwrap_or_else(|| self.text("正在分析…", "Analyzing…").into()),
                    ),
            )
            .child(
                Button::new("cancel-analysis-inline")
                    .ghost()
                    .h(px(22.))
                    .px(px(9.))
                    .rounded_full()
                    .bg(p.glass)
                    .shell_label(self.text("取消", "Cancel"))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.cancel(&CancelAnalysis, window, cx);
                    })),
            )
            .into_any_element()
    }

    pub(super) fn hover_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(hit) = &self.hover else {
            return div().into_any_element();
        };
        let p = Palette::of(cx);
        let summary = hit
            .summary
            .as_ref()
            .filter(|summary| !summary.trim().is_empty());
        let width = 300.0_f32.min((self.viewport_size.0 - 16.).max(1.));
        let pointer = (self.pointer.0, self.pointer.1 + 60.);
        let left_side = pointer.0 + width + 20. > self.viewport_size.0 - 8.;
        let mut title = div()
            .flex()
            .items_center()
            .gap(px(9.))
            .h(px(20.))
            .text_size(px(13.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM);
        if let Some(icon) = hit.k.as_deref().and_then(crate::ui_icons::contributor) {
            title = title.child(icon.text_color(p.ink).size(px(17.)));
        }
        title = title.child(div().flex_1().min_w_0().truncate().child(hit.t.clone()));
        let card = div()
            .id("evidence-tooltip")
            .test_support()
            .relative()
            .w(px(width))
            .max_h(px((self.viewport_size.1 - 84.).max(1.)))
            .overflow_hidden()
            .p(px(16.))
            .rounded(px(13.))
            .shadow(p.shadows())
            .text_color(p.ink)
            .child(crate::material::surface(
                self.backdrop_image.clone(),
                rgb(if p.dark { 0x232321 } else { 0xF8F8F4 })
                    .opacity(if p.dark { 0.94 } else { 0.96 })
                    .into(),
                13.,
                p,
            ))
            .child(title)
            .when_some(summary, |card, summary| {
                card.child(
                    div()
                        .mt(px(9.))
                        .text_size(px(16.))
                        .line_height(px(24.))
                        .font_weight(FontWeight::MEDIUM)
                        .whitespace_normal()
                        .child(summary.clone()),
                )
            });
        anchored()
            .anchor(if left_side {
                Anchor::TopRight
            } else {
                Anchor::TopLeft
            })
            .position(point(
                px(if left_side {
                    pointer.0 - 16.
                } else {
                    pointer.0 + 20.
                }),
                px((pointer.1 + 18.).max(68.)),
            ))
            .snap_to_window_with_margin(px(8.))
            .child(card)
            .into_any_element()
    }
    pub(super) fn focus_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::of(cx);
        let piece = self.selected_piece();
        let variants = piece.and_then(|piece| piece["variants"].as_array());
        let variant_id = self
            .current_tile()
            .and_then(|tile| tile.spec.variant.as_deref());
        let variant_name = variants
            .and_then(|variants| {
                variants
                    .iter()
                    .find(|variant| variant["id"].as_str() == variant_id)
            })
            .and_then(|variant| {
                variant[if self.preferences.language == Language::En {
                    "en"
                } else {
                    "name"
                }]
                .as_str()
            })
            .unwrap_or(self.text("原版", "Original"));
        let layout_tip = format!("{}：{}", self.text("版式", "Layout"), variant_name);
        let style = self.style();
        let style_name = style
            .and_then(|style| {
                style[if self.preferences.language == Language::En {
                    "en"
                } else {
                    "name"
                }]
                .as_str()
            })
            .unwrap_or("");
        let style_tip = format!("{}：{}", self.text("风格", "Style"), style_name);
        let swatch = self
            .current_tile()
            .and_then(|tile| tile.image.clone())
            .map(|image| {
                img(image)
                    .w(px(20.))
                    .h(px(20.))
                    .object_fit(ObjectFit::Cover)
                    .rounded(px(5.))
                    .into_any_element()
            })
            .unwrap_or_else(|| {
                img(style_dot(
                    style.and_then(|style| style["dot"].as_str()).unwrap_or(""),
                ))
                .w(px(20.))
                .h(px(20.))
                .into_any_element()
            });
        let mut editing = div()
            .id("focus-editing")
            .flex()
            .items_center()
            .gap(px(2.))
            .child(
                self.surface_anchors.wrap(
                    "focus-layout",
                    Button::new("focus-layout")
                        .ghost()
                        .w(px(42.))
                        .h(px(36.))
                        .p_0()
                        .rounded(px(10.))
                        .accessibility_label(layout_tip.clone())
                        .tooltip(layout_tip)
                        .disabled(variants.is_none_or(|variants| variants.len() < 2))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .child(crate::ui_icons::icon("layout"))
                                .child(crate::ui_icons::icon("down").size(px(10.))),
                        )
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.open_panel(Panel::Layouts, window, cx)
                        })),
                ),
            )
            .child(
                self.surface_anchors.wrap(
                    "focus-style",
                    Button::new("styles")
                        .ghost()
                        .w(px(42.))
                        .h(px(36.))
                        .p_0()
                        .rounded(px(10.))
                        .accessibility_label(style_tip.clone())
                        .tooltip(style_tip)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .child(swatch)
                                .child(crate::ui_icons::icon("down").size(px(10.))),
                        )
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.open_panel(Panel::Styles, window, cx)
                        })),
                ),
            );
        if self
            .focus_styles
            .as_ref()
            .is_some_and(|styles| styles.current != styles.original)
        {
            editing = editing.child(
                Button::new("apply-focus-style")
                    .ghost()
                    .w(px(36.))
                    .h(px(36.))
                    .p_0()
                    .rounded(px(10.))
                    .icon(crate::ui_icons::icon("check"))
                    .accessibility_label(self.text("设为全局风格", "Use for all cards"))
                    .tooltip(self.text("设为全局风格", "Use for all cards"))
                    .on_click(cx.listener(|view, _, _, cx| view.apply_focus_style(cx))),
            );
        }
        let mut playback = div()
            .id("focus-playback")
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .w(px(if self.preferences.language == Language::En {
                96.
            } else {
                74.
            }))
            .h(px(36.))
            .flex_shrink_0();
        if piece.is_some_and(|piece| piece["animated"] == true) {
            let playing = self.playback.as_ref().is_some_and(|play| play.playing);
            let ended = self.playback.as_ref().is_some_and(|play| play.ended);
            let name = if self.animation_busy {
                self.text("准备播放…", "Preparing playback…")
            } else if playing {
                self.text("暂停", "Pause")
            } else if ended {
                self.text("从头重播", "Replay from start")
            } else if self.playback.is_some() {
                self.text("继续播放", "Continue playback")
            } else {
                self.text("播放一次", "Play once")
            };
            playback = playback.child(
                Button::new("play")
                    .ghost()
                    .w(px(36.))
                    .h(px(36.))
                    .p_0()
                    .rounded(px(10.))
                    .icon(crate::ui_icons::icon(if playing {
                        "pause"
                    } else if ended {
                        "replay"
                    } else {
                        "play"
                    }))
                    .accessibility_label(name)
                    .tooltip(name)
                    .disabled(self.animation_busy)
                    .on_click(cx.listener(|view, _, _, cx| view.play(false, cx))),
            );
            if let Some(play) = &self.playback {
                let progress = (play.elapsed.as_secs_f32() * 1000.
                    / play.timeline.duration_ms.max(1) as f32)
                    .clamp(0., 1.);
                playback = playback.child(
                    div()
                        .id("playback-progress")
                        .absolute()
                        .bottom_0()
                        .left(px(9.))
                        .right(px(9.))
                        .h(px(2.))
                        .rounded_full()
                        .overflow_hidden()
                        .bg(p.edge)
                        .child(
                            div()
                                .h_full()
                                .w(relative(progress))
                                .rounded_full()
                                .bg(p.ink.opacity(0.65)),
                        ),
                );
            }
        } else {
            playback = playback.child(
                div()
                    .id("static-artwork")
                    .test_support()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .text_size(px(10.))
                    .text_color(p.soft)
                    .child(crate::ui_icons::icon("image").size(px(12.)))
                    .child(self.text("静态作品", "Static artwork")),
            );
        }
        let separator = || {
            div()
                .w(px(1.))
                .h(px(20.))
                .mx(px(12.))
                .flex_shrink_0()
                .bg(p.edge)
        };
        let strip = div()
            .id("focus-dock")
            .test_support()
            .relative()
            .occlude()
            .flex()
            .items_center()
            .p(px(6.))
            .rounded(px(18.))
            .max_w(px((self.viewport_size.0 - 48.).max(1.)))
            .shadow(p.shadows())
            .child(crate::material::surface(
                self.backdrop_image.clone(),
                p.glass,
                18.,
                p,
            ))
            .child(editing)
            .child(separator())
            .child(playback)
            .child(separator())
            .child(
                self.surface_anchors.wrap(
                    "focus-export",
                    Button::new("focus-export")
                        .ghost()
                        .w(px(36.))
                        .h(px(36.))
                        .p_0()
                        .rounded(px(10.))
                        .bg(p.ink)
                        .text_color(p.bg)
                        .icon(crate::ui_icons::icon("export"))
                        .accessibility_label(self.text("导出作品", "Export artwork"))
                        .tooltip(self.text("导出作品", "Export artwork"))
                        .disabled(
                            self.selected_tile
                                .is_none_or(|index| !self.can_open_export_for(index)),
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            if let Some(index) = view.selected_tile {
                                view.toggle_export(index, cx);
                            }
                        })),
                ),
            );
        let mut root = div().absolute().inset_0().child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(22.))
                .flex()
                .justify_center()
                .child(strip),
        );
        for (id, direction, left, disabled, name) in [
            (
                "previous-piece",
                -1,
                true,
                self.selected_tile.is_none_or(|index| index == 0),
                self.text("上一张", "Previous"),
            ),
            (
                "next-piece",
                1,
                false,
                self.selected_tile
                    .is_none_or(|index| index + 1 >= self.tiles.len()),
                self.text("下一张", "Next"),
            ),
        ] {
            let mut nav = div()
                .absolute()
                .top(px((self.viewport_size.1 + 60.) / 2. - 20.))
                .w(px(40.))
                .h(px(40.))
                .rounded_full()
                .shadow(p.shadows());
            nav = if left {
                nav.left(px(22.))
            } else {
                nav.right(px(22.))
            };
            if !disabled {
                root = root.child(
                    nav.child(crate::material::surface(
                        self.backdrop_image.clone(),
                        p.glass,
                        20.,
                        p,
                    ))
                    .child(
                        Button::new(id)
                            .ghost()
                            .w(px(40.))
                            .h(px(40.))
                            .p_0()
                            .rounded_full()
                            .icon(crate::ui_icons::icon(if left { "prev" } else { "next" }))
                            .accessibility_label(name)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.keyboard_navigation(false, direction, cx)
                            })),
                    ),
                );
            }
        }
        root.into_any_element()
    }

    pub(super) fn settings_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::of(cx);
        let mut content = div()
            .flex()
            .flex_col()
            .min_w_0()
            .text_size(px(13.))
            .text_color(p.ink);
        match self.settings_tab {
            "general" => {
                let mut languages = div()
                    .flex()
                    .gap(px(2.))
                    .p(px(3.))
                    .rounded_full()
                    .bg(p.glass);
                for (id, name, language) in [
                    ("zh", "中文", Language::Zh),
                    ("en", "English", Language::En),
                ] {
                    let chosen = self.preferences.language == language;
                    languages = languages.child(
                        Button::new(SharedString::from(format!("settings-language-{id}")))
                            .ghost()
                            .h(px(25.))
                            .px(px(10.))
                            .rounded_full()
                            .shell_label(name)
                            .when(chosen, |b| b.bg(p.ink).text_color(p.bg))
                            .on_click(cx.listener(move |view, _, window, cx| {
                                if view.preferences.language != language {
                                    view.toggle_language(&ToggleLanguage, window, cx);
                                }
                            })),
                    );
                }
                content = content
                    .child(heading(self.text("通用", "General"), p))
                    .child(row(self.text("语言", "Language"), languages, p));
                let mut logos = div().flex().flex_wrap().gap(px(8.));
                for (id, name) in [
                    ("star", "多彩星标 / Color star"),
                    ("star-mono", "单色星标 / Mono star"),
                    ("orb", "光球 / Orb"),
                    ("ring", "圆环 / Ring"),
                    ("color", "彩色 / Color"),
                    ("mono", "单色 / Mono"),
                ] {
                    logos = logos.child(
                        div()
                            .relative()
                            .w(px(40.))
                            .h(px(40.))
                            .rounded(px(10.))
                            .bg(linear_gradient(
                                180.,
                                linear_color_stop(rgb(0x2A2A2A), 0.),
                                linear_color_stop(rgb(0x050505), 1.),
                            ))
                            .shadow(if self.preferences.logo == id {
                                vec![
                                    BoxShadow::new(px(0.), px(0.), p.ink).spread_radius(px(3.5)),
                                    BoxShadow::new(px(0.), px(0.), p.bg).spread_radius(px(2.)),
                                ]
                            } else {
                                vec![BoxShadow::new(px(0.), px(0.), p.edge).spread_radius(px(1.))]
                            })
                            .child(
                                Button::new(id)
                                    .ghost()
                                    .w_full()
                                    .h_full()
                                    .p(px(8.))
                                    .rounded(px(10.))
                                    .accessibility_label(name)
                                    .tooltip(name)
                                    .child(if matches!(id, "star-mono" | "mono") {
                                        svg()
                                            .data(logo_image(id).bytes())
                                            .size(px(24.))
                                            .text_color(rgb(0xF4F4F2))
                                            .into_any_element()
                                    } else {
                                        img(logo_image(id)).w(px(24.)).h(px(24.)).into_any_element()
                                    })
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.preferences.logo = id.into();
                                        crate::image_assets::release(view.logo_image.clone(), cx);
                                        view.logo_image = logo_image(id);
                                        view.persist();
                                        cx.notify();
                                    })),
                            ),
                    );
                }
                content = content.child(row(self.text("程序坞图标", "App icon"), logos, p));
                content = content.child(row(
                    self.text("外观", "Appearance"),
                    Button::new("appearance")
                        .ghost()
                        .h(px(26.))
                        .rounded_full()
                        .bg(p.glass)
                        .shell_label(match self.preferences.appearance {
                            Appearance::System => self.text("跟随系统", "System"),
                            Appearance::Light => self.text("浅色", "Light"),
                            Appearance::Dark => self.text("深色", "Dark"),
                        })
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.cycle_appearance(&CycleAppearance, window, cx)
                        })),
                    p,
                ));
                content = content.child(row(
                    self.text("减少动态效果", "Reduce motion"),
                    switch("reduced-motion", self.preferences.reduced_motion, p)
                        .accessibility_label(self.text("减少动态效果", "Reduce motion"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.toggle_motion(&ToggleMotion, window, cx)
                        })),
                    p,
                ));
                content = content.child(div().flex().items_center().gap(px(16.)).py(px(10.)).border_b_1().border_color(p.edge)
                    .child(div().flex_1().min_w_0().child(div().font_weight(FontWeight::MEDIUM).child(self.text("有新提交时自动跟上", "Catch up on new commits")))
                        .child(div().mt(px(2.)).text_size(px(12.)).text_color(p.soft).child(self.text("仓库里有了新提交，就在后台增量分析一次。每个项目可以在项目设置里单独改。", "Analyze new commits in the background. Each project can override this in its settings."))))
                    .child(switch("auto-analyze", self.preferences.auto_analyze, p).accessibility_label(self.text("有新提交时自动跟上", "Catch up on new commits"))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.preferences.auto_analyze = !view.preferences.auto_analyze; view.persist();
                            if let Some(background) = &view.background { background.refresh(); } cx.notify();
                        }))));
                content = content.child(row(
                    self.text("写其他贡献者的名字", "Show contributor names"),
                    switch("names-settings", self.preferences.named, p)
                        .accessibility_label(
                            self.text("写其他贡献者的名字", "Show contributor names"),
                        )
                        .on_click(cx.listener(|view, _, _, cx| view.set_names(cx))),
                    p,
                ));
            }
            "data" => {
                content = content
                    .child(heading(self.text("本地项目库", "Library"), p))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .py(px(9.))
                            .border_b_1()
                            .border_color(p.edge)
                            .child(
                                div()
                                    .text_color(p.soft)
                                    .child(self.text("位置", "Location")),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(11.))
                                    .text_color(p.soft)
                                    .truncate()
                                    .child(crate::material::short_path(&service::library_path())),
                            )
                            .child(
                                Button::new("open-library")
                                    .ghost()
                                    .h(px(26.))
                                    .rounded_full()
                                    .bg(p.glass)
                                    .shell_label(self.text("打开", "Open"))
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.reveal_path(Path::new(&service::library_path()))
                                    })),
                            ),
                    );
                let on = self.collect["on"].as_bool().unwrap_or(false);
                let mode = if self.collect["agent"]["on"].as_bool().unwrap_or(false) {
                    "always"
                } else if on {
                    "open"
                } else {
                    "off"
                };
                let supported = self.collect["agent"]["supported"]
                    .as_bool()
                    .unwrap_or(false);
                let mut modes = div()
                    .flex()
                    .gap(px(2.))
                    .p(px(3.))
                    .rounded_full()
                    .bg(p.glass);
                for (id, label) in [
                    ("off", self.text("关", "Off")),
                    ("open", self.text("App 开着时", "While open")),
                    ("always", self.text("一直", "Always")),
                ] {
                    if id == "always" && !supported {
                        continue;
                    }
                    modes = modes.child(
                        Button::new(id)
                            .ghost()
                            .h(px(25.))
                            .px(px(9.))
                            .rounded_full()
                            .shell_label(label)
                            .when(mode == id, |b| b.bg(p.ink).text_color(p.bg))
                            .disabled(self.operation_busy)
                            .on_click(
                                cx.listener(move |view, _, _, cx| view.set_collection_mode(id, cx)),
                            ),
                    );
                }
                content = content.child(row(
                    self.text("备份 AI 工具记录", "Back up AI tool logs"),
                    modes,
                    p,
                ));
                for (label, value) in
                    crate::ui_reports::report_rows(&self.collect, &self.preferences.language)
                {
                    content = content.child(
                        div()
                            .flex()
                            .justify_between()
                            .gap(px(10.))
                            .py(px(5.))
                            .text_size(px(12.))
                            .text_color(p.soft)
                            .child(label)
                            .child(value),
                    );
                }
                content = content.child(
                    Button::new("collect-now")
                        .ghost()
                        .h(px(26.))
                        .self_start()
                        .rounded_full()
                        .bg(p.glass)
                        .shell_label(self.text("立即备份", "Back up now"))
                        .disabled(self.operation_busy)
                        .on_click(
                            cx.listener(|view, _, _, cx| view.operation(service::collect_now, cx)),
                        ),
                );
                content = content.child(heading(self.text("AI 工具", "AI tools"), p));
                if let Some(report) = &self.report {
                    for (label, value) in
                        crate::ui_reports::report_rows(report, &self.preferences.language)
                    {
                        content = content.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(10.))
                                .py(px(9.))
                                .border_b_1()
                                .border_color(p.edge)
                                .child(label)
                                .child(div().text_size(px(12.)).text_color(p.soft).child(value)),
                        );
                    }
                }
                content = content.child(
                    Button::new("diagnostics")
                        .ghost()
                        .h(px(28.))
                        .self_start()
                        .rounded_full()
                        .shell_label(self.text("检查数据源", "Check data sources"))
                        .on_click(cx.listener(|view, _, _, cx| view.request_diagnostics(cx))),
                );
            }
            _ => {
                content = content.items_center().text_center().pt(px(40.))
                    .child(img(logo_image("star")).w(px(56.)).h(px(56.)).mb(px(14.)))
                    .child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).child("creditia"))
                    .child(div().mt(px(4.)).text_size(px(12.)).text_color(p.faint).child(format!("{} {}", self.text("版本", "Version"), env!("CARGO_PKG_VERSION"))))
                    .child(div().mt(px(14.)).max_w(px(380.)).text_size(px(12.)).text_color(p.soft).child(self.text("看清每个 AI 工具在你的仓库里写了多少，再把它做成能放进 README、发版说明和作品集的署名。", "See how much each AI tool wrote in your repos, then turn it into credits for your README, releases and portfolio.")))
                    .child(div().mt(px(6.)).text_size(px(12.)).text_color(p.faint).child(self.text("只在本机运行，不上传任何代码或记录。", "Runs only on this computer. No code or logs are uploaded.")));
            }
        }
        content.into_any_element()
    }
}
