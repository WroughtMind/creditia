use super::{
    Analyze, CancelAnalysis, CancelCurrent, CancelExport, CloseWindow, CopyArchive, Creditia,
    CycleAppearance, ExportArtwork, ExportHistory, Fixture, FocusArtwork, FullScreen, HideApp,
    HideOthers, MinimizeWindow, NextLayout, NextPiece, NextStyle, OpenProject, Panel, Play,
    PreviousLayout, PreviousPiece, PreviousStyle, ProjectSettings, Quit, Refresh, Replay,
    ResetZoom, RevealProject, Roam, Settings, ShowAll, SwitchProject, ToggleLanguage, ToggleMotion,
    Wall, ZoomIn, ZoomOut, write_performance_event,
};
use crate::live_mark::MarkStatus;
use crate::material::{Palette, ShellLabel};
use crate::{Appearance, Language, Preferences, renderer};
use creditia_desktop_service as service;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

impl Creditia {
    pub(super) fn window_activity(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.fixture {
            crate::platform_ui::refresh_system_motion(cx);
        }
        let active = window.is_window_active() && self.window_visible;
        if let Some(background) = &self.background {
            background.set_active(active);
        }
        if !active {
            self.roam_motion.stop();
            self.hover = None;
            self.hovered_tile = None;
            self.drag = None;
            if let Some(control) = &self.animation_control {
                control.set_visible(false);
                control.pause();
            }
            if let Some(play) = &mut self.playback
                && play.playing
                && !play.suspended
            {
                play.suspended = true;
            }
        }
        if telemetry_enabled() {
            let mut state = self.diagnostic_state();
            state["active"] = json!(active);
            state["visible"] = json!(self.window_visible);
            state["viewport_ready"] = json!(false);
            if state["animation"].is_object() {
                state["animation"]["visible"] = json!(active && self.panel.is_none());
            }
            write_performance_event(
                json!({"event":"visibility","visible":self.window_visible,"active":active,"state":state}),
            );
            self.telemetry_state = None;
        }
        cx.notify();
    }

    pub(super) fn activity_age(&self) -> Duration {
        if self.fixture && !self.benchmark {
            Duration::from_secs(3)
        } else {
            self.activity_since
                .map(|since| since.elapsed())
                .unwrap_or_default()
        }
    }

    pub(super) fn sync_activity(&mut self, cx: &mut Context<Self>) {
        let busy = self.loading || self.job_busy || self.automatic_job.is_some();
        if busy && self.activity_since.is_none() {
            self.activity_since = Some(Instant::now());
            let executor = cx.background_executor().clone();
            self.activity_task = Some(cx.spawn(async move |this, cx| {
                executor.timer(Duration::from_millis(300)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
                executor.timer(Duration::from_millis(1700)).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            }));
        } else if !busy {
            self.activity_since = None;
            self.activity_task = None;
        }
    }

    pub(super) fn toggle_language(
        &mut self,
        _: &ToggleLanguage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preferences.language = match self.preferences.language {
            Language::Zh => Language::En,
            Language::En => Language::Zh,
        };
        let language = self.preferences.language.clone();
        self.search_input.update(cx, |input, cx| {
            input.set_placeholder(language.text("搜索项目", "Search projects"), window, cx)
        });
        self.sign_input.update(cx, |input, cx| {
            input.set_placeholder(language.text("署名", "Signature"), window, cx)
        });
        self.persist();
        install_menus(&self.preferences.language, cx);
        if self.selected.as_deref() == Some("__all__") && !self.fixture {
            self.show_all_projects(cx);
        } else {
            self.rebuild_preserving_view(cx);
        }
    }

    pub(super) fn cycle_appearance(
        &mut self,
        _: &CycleAppearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preferences.appearance = match self.preferences.appearance {
            Appearance::System => Appearance::Light,
            Appearance::Light => Appearance::Dark,
            Appearance::Dark => Appearance::System,
        };
        apply_appearance(&self.preferences.appearance, window, cx);
        self.persist();
        cx.notify();
    }

    pub(super) fn toggle_motion(
        &mut self,
        _: &ToggleMotion,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preferences.reduced_motion = !self.preferences.reduced_motion;
        self.reduced_motion = self.preferences.reduced_motion
            || (!self.fixture && crate::platform_ui::system_reduced_motion(cx).unwrap_or(false));
        cx.set_reduce_motion(self.reduced_motion);
        self.stop_playback(cx);
        if self.reduced_motion {
            self.focus_transition = None;
            self.panel_tween = None;
            if self.panel_closing {
                self.panel_closing = false;
                self.panel = None;
            }
        }
        self.persist();
        cx.notify();
    }

    pub(super) fn sync_reduced_motion(&mut self, cx: &mut Context<Self>) {
        let reduced = self.preferences.reduced_motion
            || (!self.fixture && crate::platform_ui::system_reduced_motion(cx).unwrap_or(false));
        // Kit also observes OS settings. Keep its effective value in sync even
        // when an OS change does not change our app-or-system preference.
        if cx.reduce_motion() != reduced {
            cx.set_reduce_motion(reduced);
        }
        if reduced != self.reduced_motion {
            self.reduced_motion = reduced;
            if reduced {
                self.stop_playback(cx);
                self.focus_transition = None;
                self.panel_tween = None;
                if self.panel_closing {
                    self.panel_closing = false;
                    self.panel = None;
                }
            }
        }
    }

    pub(super) fn update_platform_chrome(&mut self, window: &mut Window) {
        if !self.fixture {
            if crate::platform_ui::dynamic_logo_supported()
                && let Err(error) = self.platform_ui.apply_logo(window, &self.preferences.logo)
                && self.platform_error.as_ref() != Some(&error)
            {
                self.notice = Some(error.clone());
                self.platform_error = Some(error);
            }
            if crate::platform_ui::progress_supported() {
                let progress = if self.error.is_some() {
                    crate::platform_ui::Progress::Error
                } else if self.job_busy || self.automatic_job.is_some() {
                    self.progress
                        .as_ref()
                        .and_then(analysis_fraction)
                        .map(crate::platform_ui::Progress::Fraction)
                        .unwrap_or(crate::platform_ui::Progress::Indeterminate)
                } else if self.export_stage.is_some() {
                    crate::platform_ui::Progress::Indeterminate
                } else {
                    crate::platform_ui::Progress::None
                };
                if let Err(error) = self.platform_ui.set_progress(window, progress)
                    && self.platform_error.as_ref() != Some(&error)
                {
                    self.notice = Some(error.clone());
                    self.platform_error = Some(error);
                }
            }
        }
    }

    pub(super) fn render_chrome(
        &mut self,
        palette: Palette,
        delayed_job: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (AnyElement, AnyElement) {
        let glass_shadow = palette.shadows();
        let title = if self.selected.as_deref() == Some("__all__") {
            self.text("全部项目", "All projects").to_string()
        } else {
            self.current()
                .map(|p| p.name.clone())
                .unwrap_or_else(|| self.text("项目作品库", "Project library").into())
        };
        let style_dot = self
            .style()
            .and_then(|style| style["dot"].as_str())
            .map(style_dot);
        let style_name = self
            .style()
            .and_then(|s| {
                s[if self.preferences.language == Language::En {
                    "en"
                } else {
                    "name"
                }]
                .as_str()
            })
            .unwrap_or(self.text("风格", "Style"))
            .to_string();
        let tools = self.render_toolbar_tools(palette, glass_shadow.clone(), cx);
        let mark = if self.preferences.logo == "star" {
            self.live_mark.render(
                MarkStatus {
                    progress: self.progress.as_ref().and_then(analysis_fraction),
                    busy: self.loading || self.job_busy || self.automatic_job.is_some(),
                    error: self.error.is_some(),
                },
                self.reduced_motion,
                20.,
                window,
                cx,
            )
        } else if matches!(self.preferences.logo.as_str(), "star-mono" | "mono") {
            svg()
                .data(self.logo_image.bytes())
                .w(px(20.))
                .h(px(20.))
                .text_color(palette.ink)
                .into_any_element()
        } else {
            crate::image_assets::track(&self.logo_image, window, cx);
            img(self.logo_image.clone())
                .w(px(20.))
                .h(px(20.))
                .into_any_element()
        };
        let toolbar = div()
            .h(px(60.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .pl(px(if cfg!(target_os = "macos") { 84. } else { 58. }))
            .pr_4()
            .child(
                self.surface_anchors
                    .wrap(
                        "project-trigger",
                        Button::new("switch-project")
                            .ghost()
                            .min_w_0()
                            .flex_shrink_1()
                            .h(px(32.))
                            .pl(px(8.))
                            .pr(px(10.))
                            .rounded(px(10.))
                            .accessibility_label(title.clone())
                            .tooltip(self.mark_tooltip.clone())
                            .child(
                                div()
                                    .flex()
                                    .min_w_0()
                                    .items_center()
                                    .gap(px(8.))
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(mark)
                                    .child(div().max_w(px(340.)).min_w_0().truncate().child(title))
                                    .child(
                                        crate::ui_icons::icon("updown").text_color(palette.faint),
                                    ),
                            )
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_panel(Panel::Projects, window, cx)
                            })),
                    )
                    .min_w_0()
                    .flex_shrink_1(),
            )
            .when(delayed_job, |toolbar| {
                toolbar.child(self.analysis_progress(cx))
            })
            .child(div().flex_1())
            .child(tools);
        let toolbar = if self.focused {
            let title = self
                .current_tile()
                .and_then(|tile| self.piece(&tile.spec.piece))
                .and_then(|piece| {
                    piece[if self.preferences.language == Language::En {
                        "en"
                    } else {
                        "name"
                    }]
                    .as_str()
                })
                .unwrap_or("");
            div()
                .h(px(60.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_2()
                .pl(px(if cfg!(target_os = "macos") { 94. } else { 60. }))
                .pr_4()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(15.))
                        .child(title.to_string()),
                )
                .when(delayed_job, |toolbar| {
                    toolbar.child(self.analysis_progress(cx))
                })
                .child(div().flex_1())
                .child(
                    Button::new("focus-close-top")
                        .ghost()
                        .w(px(34.))
                        .h(px(34.))
                        .p_0()
                        .rounded_full()
                        .bg(palette.glass)
                        .icon(crate::ui_icons::icon("close"))
                        .accessibility_label(self.text("返回展览", "Return to exhibition"))
                        .tooltip(self.text("返回展览", "Return to exhibition"))
                        .on_click(cx.listener(|view, _, _, cx| view.close_focus(cx))),
                )
                .into_any_element()
        } else {
            toolbar.into_any_element()
        };
        let controls = self.render_view_controls(palette, glass_shadow, style_dot, style_name, cx);
        (toolbar, controls)
    }

    fn render_toolbar_tools(
        &self,
        palette: Palette,
        glass_shadow: Vec<BoxShadow>,
        cx: &mut Context<Self>,
    ) -> Div {
        let fg = palette.ink;
        let muted = palette.soft;
        let border = palette.edge;
        let selected = palette.selected;
        let mut tools = div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .h(px(36.))
            .gap(px(2.))
            .p(px(2.))
            .rounded_full()
            .relative()
            .shadow(glass_shadow)
            .child(crate::material::surface(
                self.backdrop_image.clone(),
                palette.glass,
                18.,
                palette,
            ))
            .border_1()
            .border_color(border);
        let group = self.group();
        let group = if group == "auto" {
            self.resolved_group.as_deref().unwrap_or("tool")
        } else {
            group.as_str()
        };
        for (id, label) in [
            ("tool", self.text("按工具", "By tool")),
            ("model", self.text("按模型", "By model")),
        ] {
            if self.has_models == Some(false) {
                continue;
            }
            let element = match id {
                "auto" => "group-auto",
                "tool" => "group-tool",
                _ => "group-model",
            };
            tools = tools.child(
                Button::new(element)
                    .ghost()
                    .accessibility_label(label)
                    .h(px(30.))
                    .px(px(11.))
                    .rounded_full()
                    .text_color(muted)
                    .when(group == id, |b| b.bg(selected).text_color(fg))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(if group == id {
                                FontWeight::MEDIUM
                            } else {
                                FontWeight::NORMAL
                            })
                            .child(label),
                    )
                    .on_click(cx.listener(move |view, _, _, cx| view.set_group(id, cx))),
            );
        }
        if self.has_models != Some(false) {
            tools = tools.child(div().w(px(1.)).h(px(16.)).mx(px(4.)).bg(border));
        }
        tools = tools
            .child(
                Button::new("wall")
                    .ghost()
                    .w(px(30.))
                    .h(px(30.))
                    .p_0()
                    .rounded_full()
                    .text_color(muted)
                    .icon(crate::ui_icons::icon("wall"))
                    .accessibility_label(self.text("展墙", "Wall"))
                    .tooltip(self.text("展墙", "Wall"))
                    .when(self.preferences.view == "wall" && !self.focused, |b| {
                        b.bg(selected).text_color(fg)
                    })
                    .on_click(cx.listener(|view, _, _, cx| view.set_view("wall", cx))),
            )
            .child(
                Button::new("roam")
                    .ghost()
                    .w(px(30.))
                    .h(px(30.))
                    .p_0()
                    .rounded_full()
                    .text_color(muted)
                    .icon(crate::ui_icons::icon("roam"))
                    .accessibility_label(self.text("漫游", "Roam"))
                    .tooltip(self.text("漫游", "Roam"))
                    .when(self.preferences.view == "roam" && !self.focused, |b| {
                        b.bg(selected).text_color(fg)
                    })
                    .on_click(cx.listener(|view, _, _, cx| view.set_view("roam", cx))),
            )
            .child(div().w(px(1.)).h(px(16.)).mx(px(4.)).bg(border))
            .child(
                Button::new("toolbar-language")
                    .ghost()
                    .w(px(30.))
                    .h(px(30.))
                    .p_0()
                    .rounded_full()
                    .text_color(muted)
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::NORMAL)
                            .child(if self.preferences.language == Language::En {
                                "EN"
                            } else {
                                "中"
                            }),
                    )
                    .accessibility_label(self.text("切换到英文", "Switch to Chinese"))
                    .tooltip(self.text("切换到英文", "Switch to Chinese"))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.toggle_language(&ToggleLanguage, window, cx)
                    })),
            )
            .child(
                self.surface_anchors.wrap(
                    "settings-trigger",
                    Button::new("settings")
                        .ghost()
                        .w(px(30.))
                        .h(px(30.))
                        .p_0()
                        .rounded_full()
                        .text_color(muted)
                        .icon(crate::ui_icons::icon("gear"))
                        .accessibility_label(self.text("设置", "Settings"))
                        .tooltip(self.text("设置", "Settings"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.open_panel(Panel::Settings, window, cx)
                        })),
                ),
            );
        tools
    }

    fn render_view_controls(
        &self,
        palette: Palette,
        glass_shadow: Vec<BoxShadow>,
        style_dot: Option<Arc<Image>>,
        style_name: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let muted = palette.soft;
        let border = palette.edge;
        let mut controls = div()
            .id("view-controls")
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.drag = None;
                    cx.stop_propagation();
                }),
            )
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(18.))
            .h(px(40.))
            .flex()
            .items_center()
            .gap_1()
            .px(px(18.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(40.))
                    .p(px(3.))
                    .rounded_full()
                    .relative()
                    .shadow(glass_shadow)
                    .child(crate::material::surface(
                        self.backdrop_image.clone(),
                        palette.glass,
                        20.,
                        palette,
                    ))
                    .border_1()
                    .border_color(border)
                    .child(
                        Button::new("style-previous")
                            .ghost()
                            .w(px(30.))
                            .h(px(32.))
                            .p_0()
                            .rounded_full()
                            .text_color(muted)
                            .icon(crate::ui_icons::icon("prev"))
                            .accessibility_label(self.text("上一种风格", "Previous style"))
                            .tooltip(self.text("上一种风格", "Previous style"))
                            .on_click(cx.listener(|view, _, _, cx| view.step_style(-1, cx))),
                    )
                    .child(
                        self.surface_anchors.wrap(
                            "style-trigger",
                            Button::new("styles")
                                .ghost()
                                .h(px(32.))
                                .pl(px(6.))
                                .pr(px(12.))
                                .rounded_full()
                                .accessibility_label(style_name.clone())
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(9.))
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .when_some(style_dot, |row, dot| {
                                            row.child(
                                                div()
                                                    .w(px(22.))
                                                    .h(px(22.))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(img(dot).w(px(18.)).h(px(18.))),
                                            )
                                        })
                                        .child(style_name)
                                        .child(crate::ui_icons::icon("updown").text_color(muted)),
                                )
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.open_panel(Panel::Styles, window, cx)
                                })),
                        ),
                    )
                    .child(
                        Button::new("style-next")
                            .ghost()
                            .w(px(30.))
                            .h(px(32.))
                            .p_0()
                            .rounded_full()
                            .text_color(muted)
                            .icon(crate::ui_icons::icon("next"))
                            .accessibility_label(self.text("下一种风格", "Next style"))
                            .tooltip(self.text("下一种风格", "Next style"))
                            .on_click(cx.listener(|view, _, _, cx| view.step_style(1, cx))),
                    ),
            )
            .child(
                self.surface_anchors.wrap(
                    "pieces-trigger",
                    Button::new("pieces")
                        .ghost()
                        .icon(crate::ui_icons::icon("more"))
                        .accessibility_label(self.text("作品", "Artworks"))
                        .tooltip(self.text("作品", "Artworks"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.open_panel(Panel::Pieces, window, cx)
                        })),
                ),
            )
            .when(!self.focused, |controls| controls.child(div().flex_1()));
        if self.preferences.view == "roam" && !self.focused {
            controls = controls.child(div().flex_1()).child(
                Button::new("zoom-reset")
                    .ghost()
                    .h(px(40.))
                    .px(px(16.))
                    .rounded_full()
                    .bg(palette.glass)
                    .icon(crate::ui_icons::icon("home"))
                    .shell_label(self.text("回到原点", "Back to origin"))
                    .on_click(cx.listener(|view, _, _, cx| view.reset_viewport(cx))),
            );
        }
        if self.focused {
            self.focus_controls(cx)
        } else {
            controls.into_any_element()
        }
    }
}

pub(super) fn apply_appearance(appearance: &Appearance, window: &mut Window, cx: &mut App) {
    match appearance {
        Appearance::System => Theme::sync_system_appearance(Some(window), cx),
        Appearance::Light => Theme::change(ThemeMode::Light, Some(window), cx),
        Appearance::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
    }
    Theme::update(cx, crate::material::configure_theme);
}
pub(super) fn install_menus(language: &Language, cx: &mut App) {
    cx.set_menus(vec![
        Menu {
            name: "Creditia".into(),
            disabled: false,
            items: vec![
                MenuItem::action(language.text("设置…", "Settings…"), Settings),
                MenuItem::action(language.text("切换语言", "Switch language"), ToggleLanguage),
                MenuItem::action(
                    language.text("切换明暗主题", "Cycle appearance"),
                    CycleAppearance,
                ),
                MenuItem::action(
                    language.text("减少动态效果", "Reduced motion"),
                    ToggleMotion,
                ),
                MenuItem::separator(),
                MenuItem::action(language.text("隐藏 Creditia", "Hide Creditia"), HideApp),
                MenuItem::action(language.text("隐藏其他应用", "Hide Others"), HideOthers),
                MenuItem::action(language.text("显示全部", "Show All"), ShowAll),
                MenuItem::separator(),
                MenuItem::action(language.text("退出 Creditia", "Quit Creditia"), Quit),
            ],
        },
        Menu {
            name: language.text("项目", "Project").into(),
            disabled: false,
            items: vec![
                MenuItem::action(language.text("切换项目…", "Switch project…"), SwitchProject),
                MenuItem::action(language.text("添加文件夹…", "Add folder…"), OpenProject),
                MenuItem::action(language.text("重新分析", "Re-analyze"), Analyze),
                MenuItem::action(language.text("取消分析", "Cancel analysis"), CancelAnalysis),
                MenuItem::action(
                    language.text("项目设置…", "Project settings…"),
                    ProjectSettings,
                ),
                MenuItem::separator(),
                MenuItem::action(language.text("复制档案", "Copy archive"), CopyArchive),
                MenuItem::action(
                    language.text("打开所在位置", "Reveal folder"),
                    RevealProject,
                ),
            ],
        },
        Menu {
            name: language.text("编辑", "Edit").into(),
            disabled: false,
            items: vec![
                MenuItem::action(
                    language.text("撤销", "Undo"),
                    gpui_kit::component::input::Undo,
                ),
                MenuItem::action(
                    language.text("重做", "Redo"),
                    gpui_kit::component::input::Redo,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    language.text("剪切", "Cut"),
                    gpui_kit::component::input::Cut,
                ),
                MenuItem::action(
                    language.text("复制", "Copy"),
                    gpui_kit::component::input::Copy,
                ),
                MenuItem::action(
                    language.text("粘贴", "Paste"),
                    gpui_kit::component::input::Paste,
                ),
                MenuItem::action(
                    language.text("全选", "Select all"),
                    gpui_kit::component::input::SelectAll,
                ),
            ],
        },
        Menu {
            name: language.text("视图", "View").into(),
            disabled: false,
            items: vec![
                MenuItem::action(language.text("展墙", "Wall"), Wall),
                MenuItem::action(language.text("漫游", "Roam"), Roam),
                MenuItem::action(language.text("上一种风格", "Previous style"), PreviousStyle),
                MenuItem::action(language.text("下一种风格", "Next style"), NextStyle),
                MenuItem::separator(),
                MenuItem::action(language.text("放大", "Zoom in"), ZoomIn),
                MenuItem::action(language.text("缩小", "Zoom out"), ZoomOut),
                MenuItem::action(language.text("重置缩放", "Reset zoom"), ResetZoom),
                MenuItem::separator(),
                MenuItem::action(language.text("播放／暂停", "Play / pause"), Play),
                MenuItem::action(language.text("重播", "Replay"), Replay),
            ],
        },
        Menu {
            name: language.text("窗口", "Window").into(),
            disabled: false,
            items: vec![
                MenuItem::action(language.text("关闭窗口", "Close Window"), CloseWindow),
                MenuItem::action(language.text("最小化", "Minimize"), MinimizeWindow),
                MenuItem::action(language.text("切换全屏", "Toggle Full Screen"), FullScreen),
            ],
        },
        Menu {
            name: language.text("导出", "Export").into(),
            disabled: false,
            items: vec![
                MenuItem::action(language.text("导出作品…", "Export artwork…"), ExportArtwork),
                MenuItem::action(language.text("取消导出", "Cancel export"), CancelExport),
                MenuItem::separator(),
                MenuItem::action(language.text("导出记录…", "Export history…"), ExportHistory),
            ],
        },
    ]);
}
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys(key_bindings(if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    }));
}
pub(super) fn key_bindings(modifier: &str) -> Vec<KeyBinding> {
    const SHORTCUTS: &[(&str, &str, &str)] = &[
        ("q", "cmd-q", "ctrl-q"),
        ("h", "cmd-h", "ctrl-h"),
        ("alt-h", "cmd-alt-h", "ctrl-alt-h"),
        ("w", "cmd-w", "ctrl-w"),
        ("m", "cmd-m", "ctrl-m"),
        ("k", "cmd-k", "ctrl-k"),
        ("o", "cmd-o", "ctrl-o"),
        ("r", "cmd-r", "ctrl-r"),
        ("i", "cmd-i", "ctrl-i"),
        (",", "cmd-,", "ctrl-,"),
        ("1", "cmd-1", "ctrl-1"),
        ("2", "cmd-2", "ctrl-2"),
        ("[", "cmd-[", "ctrl-["),
        ("]", "cmd-]", "ctrl-]"),
        ("e", "cmd-e", "ctrl-e"),
        ("shift-r", "cmd-shift-r", "ctrl-shift-r"),
        ("shift-c", "cmd-shift-c", "ctrl-shift-c"),
        ("shift-space", "cmd-shift-space", "ctrl-shift-space"),
        ("=", "cmd-=", "ctrl-="),
        ("-", "cmd--", "ctrl--"),
        ("0", "cmd-0", "ctrl-0"),
    ];
    let shortcut = |key: &str| -> &'static str {
        let row = SHORTCUTS
            .iter()
            .find(|row| row.0 == key)
            .expect("registered shortcut");
        if modifier == "cmd" { row.1 } else { row.2 }
    };
    vec![
        KeyBinding::new(shortcut("q"), Quit, None),
        KeyBinding::new(shortcut("h"), HideApp, Some("Creditia && !Input")),
        KeyBinding::new(shortcut("alt-h"), HideOthers, Some("Creditia && !Input")),
        KeyBinding::new(shortcut("w"), CloseWindow, Some("Creditia")),
        KeyBinding::new(shortcut("m"), MinimizeWindow, Some("Creditia && !Input")),
        KeyBinding::new(shortcut("k"), SwitchProject, Some("Creditia")),
        KeyBinding::new(shortcut("o"), OpenProject, Some("Creditia")),
        KeyBinding::new(shortcut("r"), Analyze, Some("Creditia")),
        KeyBinding::new(shortcut("i"), ProjectSettings, Some("Creditia")),
        KeyBinding::new(shortcut(","), Settings, Some("Creditia")),
        KeyBinding::new(shortcut("1"), Wall, Some("Creditia")),
        KeyBinding::new(shortcut("2"), Roam, Some("Creditia")),
        KeyBinding::new(shortcut("["), PreviousStyle, Some("Creditia")),
        KeyBinding::new(shortcut("]"), NextStyle, Some("Creditia")),
        KeyBinding::new(shortcut("e"), ExportArtwork, Some("Creditia")),
        KeyBinding::new(shortcut("shift-r"), Refresh, Some("Creditia")),
        KeyBinding::new(shortcut("shift-c"), CopyArchive, Some("Creditia")),
        KeyBinding::new("escape", CancelCurrent, Some("Creditia")),
        KeyBinding::new("escape", CancelCurrent, Some("CreditiaExport")),
        KeyBinding::new(shortcut("w"), CloseWindow, Some("CreditiaExport")),
        KeyBinding::new(shortcut("m"), MinimizeWindow, Some("CreditiaExport")),
        KeyBinding::new(shortcut("e"), ExportArtwork, Some("CreditiaExport")),
        KeyBinding::new(shortcut("="), ZoomIn, Some("Creditia")),
        KeyBinding::new(shortcut("-"), ZoomOut, Some("Creditia")),
        KeyBinding::new(shortcut("0"), ResetZoom, Some("Creditia")),
        KeyBinding::new("home", ResetZoom, Some("Creditia && !CreditiaModal")),
        KeyBinding::new("0", ResetZoom, Some("Creditia && !CreditiaModal")),
        KeyBinding::new("up", PreviousPiece, Some("Creditia && !CreditiaModal")),
        KeyBinding::new("down", NextPiece, Some("Creditia && !CreditiaModal")),
        KeyBinding::new("enter", FocusArtwork, Some("Creditia && !CreditiaModal")),
        KeyBinding::new("space", Play, Some("Creditia && !CreditiaModal")),
        KeyBinding::new(
            shortcut("shift-space"),
            Replay,
            Some("Creditia && !CreditiaModal"),
        ),
        KeyBinding::new("left", PreviousLayout, Some("Creditia && !CreditiaModal")),
        KeyBinding::new("right", NextLayout, Some("Creditia && !CreditiaModal")),
    ]
}
pub fn init_fonts(cx: &mut App) -> Result<(), String> {
    let files = renderer::ui_font_files()?;
    let bytes = files
        .into_iter()
        .map(|path| {
            std::fs::read(&path)
                .map(std::borrow::Cow::Owned)
                .map_err(|error| format!("Cannot load {}: {error}", path.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    cx.text_system()
        .add_fonts(bytes)
        .map_err(|error| error.to_string())?;
    Theme::update(cx, crate::material::configure_theme);
    Ok(())
}
pub fn run() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|s| s == "--version") {
        println!("Creditia {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args
        .iter()
        .any(|s| s == "--check-resources" || s == "--self-test")
    {
        let result = (|| -> Result<Value, String> {
            let catalog = renderer::request(
                json!({"version":1,"id":"resources","op":"catalog"}),
                renderer::Cancellation::default(),
            )?;
            let core = service::core_bin();
            let status = std::process::Command::new(&core)
                .arg("--help")
                .output()
                .map_err(|e| e.to_string())?;
            if !status.status.success() {
                return Err("Analysis executable failed to launch".into());
            }
            Ok(json!({"ok":true,"styles":catalog["styles"].as_array().map(Vec::len),"core":core}))
        })();
        match result {
            Ok(value) => println!("{value}"),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    let fixture = args
        .iter()
        .position(|s| s == "--fixture" || s == "--acceptance-fixture")
        .and_then(|i| args.get(i + 1))
        .map(|path| {
            std::fs::read(path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| {
                    serde_json::from_slice::<Fixture>(&bytes).map_err(|e| e.to_string())
                })
        });
    let fixture = match fixture {
        Some(Ok(fixture)) => Some(fixture),
        Some(Err(error)) => {
            eprintln!("Invalid fixture: {error}");
            std::process::exit(2);
        }
        None => None,
    };
    let benchmark = args
        .iter()
        .position(|s| s == "--benchmark-archive")
        .and_then(|i| args.get(i + 1))
        .map(|path| {
            std::fs::read(path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| {
                    serde_json::from_slice::<Value>(&bytes).map_err(|e| e.to_string())
                })
        });
    let benchmark = match benchmark {
        Some(Ok(raw)) => Some(raw),
        Some(Err(error)) => {
            eprintln!("Invalid benchmark archive: {error}");
            std::process::exit(2);
        }
        None => None,
    };
    let acceptance = benchmark.is_some() || args.iter().any(|s| s == "--acceptance-fixture");
    let application = application().with_assets(assets::Assets);
    let reopen_fixture = fixture.clone();
    let reopen_benchmark = benchmark.clone();
    application.on_reopen(move |cx| {
        if let Some(handle) = cx.windows().first().copied() {
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        } else {
            open_creditia_window(
                cx,
                reopen_fixture.clone(),
                reopen_benchmark.clone(),
                acceptance,
                None,
            );
        }
        cx.activate(true);
    });
    application.run(move |cx| {
        init(cx);
        let font_error = init_fonts(cx).err();
        cx.on_action(|_: &HideApp, cx| cx.hide());
        cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
        cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
        cx.on_action(|_: &Quit, cx| {
            service::cancel_analyze();
            cx.quit();
        });
        let closed = cx.on_window_closed(|cx, _| {
            if !cfg!(target_os = "macos") && cx.windows().is_empty() {
                cx.quit();
            }
        });
        cx.set_global(DesktopLifetime { _closed: closed });
        open_creditia_window(cx, fixture, benchmark, acceptance, font_error);
        cx.activate(true);
    });
}

struct DesktopLifetime {
    _closed: Subscription,
}
impl Global for DesktopLifetime {}
fn open_creditia_window(
    cx: &mut App,
    fixture: Option<Fixture>,
    benchmark: Option<Value>,
    acceptance: bool,
    font_error: Option<String>,
) {
    open_window(
        WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some("creditia".into()),
                // Match the original macOS overlay: controls share the 60 px toolbar.
                appears_transparent: cfg!(target_os = "macos"),
                traffic_light_position: cfg!(target_os = "macos").then(|| point(px(22.), px(23.))),
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1360.), px(880.)),
                cx,
            ))),
            window_min_size: Some(size(px(760.), px(560.))),
            ..Default::default()
        },
        cx,
        move |window, cx| {
            cx.new(|cx| {
                let mut view = if let Some(raw) = benchmark {
                    Creditia::benchmark(
                        raw,
                        Preferences {
                            language: Language::En,
                            appearance: Appearance::Light,
                            named: false,
                            ..Default::default()
                        },
                        window,
                        cx,
                    )
                } else {
                    match fixture {
                        Some(fixture) => {
                            let mut view = Creditia::fixture(fixture, window, cx);
                            view.benchmark = acceptance;
                            view
                        }
                        None => Creditia::new(window, cx),
                    }
                };
                if let Some(error) = font_error {
                    view.error = Some(error);
                }
                view
            })
        },
    )
    .expect("Unable to open Creditia");
}

pub(super) fn style_dot(svg: &str) -> Arc<Image> {
    static DOTS: std::sync::OnceLock<std::sync::Mutex<BTreeMap<String, Arc<Image>>>> =
        std::sync::OnceLock::new();
    let mut dots = DOTS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    dots.entry(svg.to_string())
        .or_insert_with(|| {
            Arc::new(Image::from_bytes(
                ImageFormat::Svg,
                format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">{svg}</svg>"#).into_bytes(),
            ))
        })
        .clone()
}

pub(super) fn bottom_fade(dark: bool) -> Arc<Image> {
    static LIGHT: std::sync::OnceLock<Arc<Image>> = std::sync::OnceLock::new();
    static DARK: std::sync::OnceLock<Arc<Image>> = std::sync::OnceLock::new();
    (if dark { &DARK } else { &LIGHT }).get_or_init(|| {
        let color = if dark { "#0A0A0A" } else { "#EFEFED" };
        let svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="80"><defs><linearGradient id="fade" x1="0" y1="0" x2="0" y2="1"><stop stop-color="{color}" stop-opacity="0"/><stop offset="1" stop-color="{color}" stop-opacity=".88"/></linearGradient></defs><path fill="url(#fade)" d="M0 0h4v80H0z"/></svg>"##);
        Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))
    }).clone()
}

pub(super) fn logo_image(id: &str) -> Arc<Image> {
    let svg = match id {
        "star-mono" => {
            include_str!("../../../../packages/render/assets/creditia/logo-star-mono.svg")
        }
        "orb" => include_str!("../../../../packages/render/assets/creditia/logo-orb.svg"),
        "ring" => include_str!("../../../../packages/render/assets/creditia/logo-ring.svg"),
        "color" => include_str!("../../../../packages/render/assets/creditia/logo-color.svg"),
        "mono" => include_str!("../../../../packages/render/assets/creditia/logo.svg"),
        _ => include_str!("../../../../packages/render/assets/creditia/logo-star.svg"),
    };
    Arc::new(Image::from_bytes(ImageFormat::Svg, svg.as_bytes().to_vec()))
}

pub(super) fn telemetry_enabled() -> bool {
    std::env::var_os("CREDITIA_APP_EVENTS").is_some()
        || std::env::var_os("CREDITIA_PERF_EVENTS").is_some()
}

pub(super) fn analysis_fraction(progress: &Value) -> Option<f64> {
    let (start, end) = match progress["stage"].as_str()? {
        "history" => (0., 0.1),
        "logs" => (0.1, 0.3),
        "attribute" => (0.3, 0.85),
        "blame" => (0.85, 1.),
        _ => return None,
    };
    let total = progress["total"].as_f64().unwrap_or(0.);
    let fraction = if total > 0. {
        progress["done"].as_f64().unwrap_or(0.) / total
    } else {
        0.
    };
    Some(start + (end - start) * fraction.clamp(0., 1.))
}

pub(super) fn project_date(stamp: i64, language: &Language) -> String {
    let days = stamp.div_euclid(86400) + 719468;
    let era = days.div_euclid(146097);
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    if *language == Language::Zh {
        format!("{year}/{month}/{day}")
    } else {
        format!("{month}/{day}/{year}")
    }
}
