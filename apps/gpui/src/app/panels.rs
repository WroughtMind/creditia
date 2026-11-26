use super::chrome::style_dot;
use super::{CopyArchive, Creditia, OpenProject, Panel, Tween};
use crate::material::{Palette, ShellLabel};
use crate::ui_controls::{choice_button, icon_button, row_button};
use crate::{Language, Project};
use creditia_desktop_service as service;
use gpui_kit::TestSupportExt as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme, Disableable, FocusTrapElement};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde_json::Value;
use std::time::Instant;

impl Creditia {
    pub(super) fn open_panel(&mut self, panel: Panel, window: &mut Window, cx: &mut Context<Self>) {
        if self.export_workspace_open() {
            self.close_export_workspace(cx);
        }
        self.roam_motion.stop();
        self.drag = None;
        self.hovered_tile = None;
        self.project_row_menu = None;
        if panel == Panel::Project {
            self.settings_project = self.selected.clone();
            self.settings_details = None;
        }
        if self.panel == Some(panel) && !self.panel_closing {
            self.close_panel(window, cx);
            return;
        }
        let from = self
            .panel_tween
            .map(Tween::value)
            .unwrap_or(if self.panel.is_some() { 1. } else { 0. });
        if self.panel.is_none() {
            self.return_focus = window.focused(cx);
        }
        self.panel = Some(panel);
        self.panel_closing = false;
        self.panel_tween = (!self.reduced_motion).then_some(Tween {
            from,
            to: 1.,
            started: Instant::now(),
        });
        self.panel_focus.focus(window, cx);
        self.remove_confirm = false;
        self.hover = None;
        if panel == Panel::Project && self.panel.is_some() {
            let sign = self
                .settings_data()
                .and_then(|d| d["config"]["display"]["sign"].as_str())
                .unwrap_or("")
                .to_string();
            self.sign_input = cx.new(|cx| InputState::new(window, cx).default_value(sign));
            self.sign_loaded_for = self.details.as_ref().and(self.selected.clone());
        }
        if panel == Panel::Projects {
            self.picker_selected = None;
            self.search_query.clear();
            self.search_input
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.project_scroll.scroll_to_item(0);
            self.search_input
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
            self.discover_projects(cx);
            self.fill_project_marks(cx);
        }
        if panel == Panel::Settings && !self.fixture {
            self.collect = service::collect_info();
        }
        cx.notify();
    }

    pub(super) fn close_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.row_mark_generation = self.row_mark_generation.wrapping_add(1);
        if let Some(cancel) = self.row_mark_cancellation.take() {
            cancel.cancel();
        }
        if self.reduced_motion {
            self.panel = None;
            self.panel_tween = None;
            self.panel_closing = false;
            self.return_focus
                .take()
                .unwrap_or_else(|| self.focus.clone())
                .focus(window, cx);
        } else {
            let from = self.panel_tween.map(Tween::value).unwrap_or(1.);
            self.panel_tween = Some(Tween {
                from,
                to: 0.,
                started: Instant::now(),
            });
            self.panel_closing = true;
        }
        cx.notify();
    }

    pub(super) fn settings_id(&self) -> Option<&String> {
        self.settings_project
            .as_ref()
            .filter(|_| self.panel == Some(Panel::Project))
            .or(self.selected.as_ref())
            .filter(|id| id.as_str() != "__all__")
    }

    pub(super) fn settings_current(&self) -> Option<&Project> {
        let id = self.settings_id()?;
        self.projects.iter().find(|p| &p.id == id)
    }

    pub(super) fn settings_data(&self) -> Option<&Value> {
        if self.settings_project.is_some() && self.settings_project != self.selected {
            self.settings_details.as_ref()
        } else {
            self.details.as_ref()
        }
    }

    pub(super) fn open_project_settings(
        &mut self,
        id: String,
        remove: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_panel(Panel::Project, window, cx);
        self.settings_project = Some(id);
        self.settings_details = None;
        self.sign_loaded_for = None;
        self.remove_confirm = remove;
        self.fetch_details(cx);
        cx.notify();
    }

    pub(super) fn picker_items(&self, cx: &App) -> Vec<String> {
        let q = self.search_input.read(cx).value().trim().to_lowercase();
        let mut items = Vec::new();
        if self.projects.len() > 1
            && (q.is_empty()
                || self
                    .text("全部项目", "All projects")
                    .to_lowercase()
                    .contains(&q)
                || "all".contains(&q))
        {
            items.push("__all__".into());
        }
        items.extend(
            self.projects
                .iter()
                .filter(|p| {
                    q.is_empty() || format!("{} {}", p.name, p.path).to_lowercase().contains(&q)
                })
                .map(|p| p.id.clone()),
        );
        items.extend(
            self.discovered
                .iter()
                .flatten()
                .filter(|p| {
                    q.is_empty()
                        || format!(
                            "{} {}",
                            p["name"].as_str().unwrap_or(""),
                            p["path"].as_str().unwrap_or("")
                        )
                        .to_lowercase()
                        .contains(&q)
                })
                .filter_map(|p| p["path"].as_str().map(|path| format!("path:{path}"))),
        );
        items
    }

    fn move_picker(&mut self, delta: isize, cx: &mut Context<Self>) {
        let items = self.picker_items(cx);
        if items.is_empty() {
            return;
        }
        let at = self
            .picker_selected
            .as_ref()
            .and_then(|key| items.iter().position(|item| item == key));
        let next = at
            .map(|at| (at as isize + delta).rem_euclid(items.len() as isize) as usize)
            .unwrap_or(if delta > 0 { 0 } else { items.len() - 1 });
        self.picker_selected = Some(items[next].clone());
        self.project_row_menu = None;
        self.project_scroll.scroll_to_item(next);
        cx.notify();
    }

    pub(super) fn activate_picker(&mut self, cx: &mut Context<Self>) {
        let items = self.picker_items(cx);
        let Some(id) = self
            .picker_selected
            .clone()
            .filter(|id| items.contains(id))
            .or_else(|| items.first().cloned())
        else {
            return;
        };
        self.project_row_menu = None;
        self.panel = None;
        if id == "__all__" {
            self.show_all_projects(cx);
        } else if let Some(path) = id.strip_prefix("path:") {
            self.queue_projects([path.into()], cx);
        } else {
            self.select(id, cx);
        }
    }

    pub(super) fn sync_panel_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.panel.is_none()
            && !self.panel_closing
            && let Some(focus) = self.return_focus.take()
        {
            focus.focus(window, cx);
        }
        if self.panel == Some(Panel::Project)
            && self.settings_data().is_some()
            && self.sign_loaded_for.as_ref() != self.settings_id()
        {
            let sign = self
                .settings_data()
                .and_then(|d| d["config"]["display"]["sign"].as_str())
                .unwrap_or("")
                .to_string();
            self.sign_input = cx.new(|cx| InputState::new(window, cx).default_value(sign));
            self.sign_loaded_for = self.settings_id().cloned();
        }
    }

    pub(super) fn animate_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tween) = self.panel_tween {
            if tween.done() {
                self.panel_tween = None;
                if self.panel_closing {
                    self.panel = None;
                    self.panel_closing = false;
                    self.return_focus
                        .take()
                        .unwrap_or_else(|| self.focus.clone())
                        .focus(window, cx);
                }
            } else {
                window.request_animation_frame();
            }
        }
    }

    #[cfg(feature = "visual-test")]
    pub fn fixture_panel_images_ready(&self, window: &mut Window, cx: &mut App) -> bool {
        match self.panel {
            Some(Panel::Settings) => self.settings_previews_ready(window, cx),
            Some(Panel::Styles) => self
                .catalog
                .as_ref()
                .and_then(|catalog| catalog["styles"].as_array())
                .into_iter()
                .flatten()
                .filter_map(|style| style["dot"].as_str())
                .all(|dot| style_dot(dot).get_render_image(window, cx).is_some()),
            Some(Panel::Projects) => self
                .project_marks
                .values()
                .all(|mark| mark.image.clone().get_render_image(window, cx).is_some()),
            _ => true,
        }
    }

    pub(super) fn render_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let panel = self.panel?;
        let title = match panel {
            Panel::Projects => self.text("项目作品库", "Project library"),
            Panel::Styles => self.text("风格", "Styles"),
            Panel::Layouts => self.text("版式", "Layout"),
            Panel::Pieces => self.text("全部作品", "All artworks"),
            Panel::Blame => self.text("逐行归因", "Line attribution"),
            Panel::Exports => self.text("导出记录", "Export history"),
            Panel::Settings => self.text("设置", "Settings"),
            Panel::Project => self.text("项目设置", "Project settings"),
            Panel::Details => self.text("归因与证据", "Attribution & evidence"),
        };
        let muted = cx.theme().muted_foreground;
        let body = div().flex().flex_col().gap(px(6.)).text_size(px(13.));
        let body = match panel {
            Panel::Projects => self.render_project_library(body, muted, cx),
            Panel::Styles => self.render_style_picker(body, cx),
            Panel::Layouts => self.render_layout_picker(body, cx),
            Panel::Pieces => self.render_piece_picker(body, cx),
            Panel::Exports => self.render_export_history(body, muted, cx),
            Panel::Blame => self.render_blame_panel(body, muted, cx),
            Panel::Settings => body.child(self.settings_content(cx)),
            Panel::Project => self.render_project_settings(body, muted, cx),
            Panel::Details => self.render_details_panel(body, muted, cx),
        };
        let palette = Palette::of(cx);
        let openness = self.panel_tween.map(Tween::value).unwrap_or(1.);
        let window_height = self.viewport_size.1 + 60.;
        let window_width = self.viewport_size.0;
        let popover = matches!(
            panel,
            Panel::Projects | Panel::Styles | Panel::Layouts | Panel::Pieces | Panel::Exports
        );
        let desired_width: f32 = match panel {
            Panel::Projects => 440.,
            Panel::Styles => 400.,
            Panel::Layouts => 236.,
            Panel::Exports => 460.,
            Panel::Pieces => 400.,
            Panel::Settings => 720.,
            _ => 560.,
        };
        let width = desired_width.min((window_width - 40.).max(1.));
        let max_height = match panel {
            Panel::Styles | Panel::Layouts | Panel::Pieces => {
                (window_height * 0.7).min(520.).min(window_height - 88.)
            }
            Panel::Projects => window_height - 72.,
            Panel::Exports => window_height - 88.,
            Panel::Settings => (window_height * 0.86).min(560.),
            _ => window_height * 0.86,
        }
        .max(1.);
        let mut modal = div()
            .id("panel")
            .test_support()
            .track_focus(&self.panel_focus)
            .relative()
            .occlude()
            .key_context("CreditiaModal")
            .capture_action(
                cx.listener(|view, _: &gpui_kit::component::input::MoveUp, _, cx| {
                    if view.panel == Some(Panel::Projects) {
                        view.move_picker(-1, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .capture_action(
                cx.listener(|view, _: &gpui_kit::component::input::MoveDown, _, cx| {
                    if view.panel == Some(Panel::Projects) {
                        view.move_picker(1, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .on_click(|_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .opacity(openness)
            .w(px(width))
            .max_h(px(max_height))
            .flex()
            .flex_col()
            .rounded(px(20.))
            .shadow(palette.shadows())
            .text_color(palette.ink)
            .text_size(px(13.))
            .child(crate::material::surface(
                self.backdrop_image.clone(),
                palette.solid,
                20.,
                palette,
            ));
        if panel == Panel::Settings {
            let navigation = self.render_settings_navigation(title, palette, cx);
            modal = modal.h(px(max_height)).child(
                div().size_full().flex().min_h_0().child(navigation).child(
                    div()
                        .id("panel-body")
                        .test_support()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .overflow_y_scroll()
                        .px(px(24.))
                        .pt(px(18.))
                        .pb(px(22.))
                        .child(body),
                ),
            );
        } else {
            if !popover {
                let heading = if panel == Panel::Project {
                    self.settings_current()
                        .map(|project| project.name.clone())
                        .unwrap_or_else(|| title.to_string())
                } else {
                    title.to_string()
                };
                let mut header = div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .px(px(24.))
                    .pt(px(20.))
                    .pb(px(6.))
                    .flex_shrink_0()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(heading),
                    );
                header = header.child(
                    icon_button("panel-close", "close", self.text("关闭", "Close"))
                        .track_focus(&self.panel_close_focus)
                        .on_click(cx.listener(|view, _, window, cx| view.close_panel(window, cx))),
                );
                modal = modal.child(header);
            }
            if panel == Panel::Projects {
                modal = modal.child(
                    div()
                        .id("project-search")
                        .test_support()
                        .track_focus(&self.search_input.read(cx).focus_handle(cx))
                        .mx(px(6.))
                        .px(px(6.))
                        .border_b_1()
                        .border_color(palette.edge)
                        .mb(px(6.))
                        .child(
                            Input::new(&self.search_input)
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false),
                        ),
                );
            }
            modal = modal.child(
                div()
                    .id("panel-body")
                    .test_support()
                    .when(panel != Panel::Projects, |body| body.overflow_y_scroll())
                    .min_h_0()
                    .when(matches!(panel, Panel::Styles | Panel::Pieces), |body| {
                        body.flex_1()
                    })
                    .max_h(px((max_height - if popover { 0. } else { 68. }).max(1.)))
                    .px(px(if popover { 6. } else { 24. }))
                    .pt(px(if popover { 6. } else { 0. }))
                    .pb(px(if popover { 6. } else { 22. }))
                    .child(body),
            );
            if panel == Panel::Projects {
                modal = modal.child(
                    div()
                        .mx(px(6.))
                        .pt(px(6.))
                        .pb(px(6.))
                        .border_t_1()
                        .border_color(palette.edge)
                        .flex_shrink_0()
                        .child(
                            row_button(
                                "add-project",
                                self.text("添加文件夹…", "Add folder…"),
                                "addFolder",
                            )
                            .rounded(px(10.))
                            .disabled(self.job_busy)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_project(&OpenProject, window, cx)
                            })),
                        ),
                );
            }
        }
        if panel == Panel::Settings {
            modal = modal.child(
                icon_button("panel-close", "close", self.text("关闭", "Close"))
                    .track_focus(&self.panel_close_focus)
                    .absolute()
                    .top(px(12.))
                    .right(px(12.))
                    .on_click(cx.listener(|view, _, window, cx| view.close_panel(window, cx))),
            );
        }
        if popover {
            modal = modal.on_mouse_down_out(cx.listener(
                move |view, event: &MouseDownEvent, window, cx| {
                    let triggers: &[&str] = if view.focused {
                        &["focus-style", "focus-layout"]
                    } else {
                        &[
                            "project-trigger",
                            "style-trigger",
                            "pieces-trigger",
                            "settings-trigger",
                        ]
                    };
                    if triggers
                        .iter()
                        .any(|trigger| view.surface_anchors.contains(trigger, event.position))
                        || (panel == Panel::Projects
                            && view.project_row_menu.is_some()
                            && view
                                .surface_anchors
                                .contains("project-row-menu", event.position))
                    {
                        return;
                    }
                    if view.panel == Some(panel) {
                        view.close_panel(window, cx);
                        view.drag = None;
                        cx.stop_propagation();
                    }
                },
            ));
        } else {
            modal = modal.mt(px((1. - openness) * 8.));
        }
        let row_menu = self.render_project_row_menu(panel, cx);
        let mut layer = div().id("panel-backdrop").absolute().inset_0();
        if !popover {
            layer = layer
                .occlude()
                .bg(rgba(0x0000002E))
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|view, _, _, cx| {
                        view.drag = None;
                        cx.stop_propagation();
                    }),
                )
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|view, _, window, cx| view.close_panel(window, cx)));
        }
        let modal = if popover {
            let (anchor, position) = match panel {
                Panel::Projects => (
                    Anchor::TopLeft,
                    point(
                        self.surface_anchors
                            .get("project-trigger")
                            .map(|bounds| bounds.left())
                            .unwrap_or(px(84.)),
                        px(54. - (1. - openness) * 6.),
                    ),
                ),
                Panel::Exports => (Anchor::TopRight, point(px(window_width - 16.), px(66.))),
                Panel::Styles | Panel::Layouts if self.focused => {
                    let bounds = self.surface_anchors.get(if panel == Panel::Layouts {
                        "focus-layout"
                    } else {
                        "focus-style"
                    });
                    (
                        Anchor::BottomLeft,
                        point(
                            px(bounds
                                .map(|bounds| f32::from(bounds.left()) - 6.)
                                .unwrap_or((window_width - width) / 2.)),
                            px(bounds
                                .map(|bounds| f32::from(bounds.top()) - 12.)
                                .unwrap_or(window_height - 78.)),
                        ),
                    )
                }
                _ => (Anchor::BottomLeft, point(px(18.), px(window_height - 68.))),
            };
            anchored()
                .anchor(anchor)
                .position(position)
                .snap_to_window_with_margin(px(18.))
                .child(modal)
                .into_any_element()
        } else {
            modal
                .focus_trap("creditia-panel", &self.panel_focus)
                .into_any_element()
        };
        Some(layer.child(modal).children(row_menu).into_any_element())
    }

    fn render_style_picker(&self, body: Div, cx: &mut Context<Self>) -> Div {
        let palette = Palette::of(cx);
        let column_width = (400.0f32.min((self.viewport_size.0 - 40.).max(1.)) - 13.) / 2.;
        let mut choices = div().flex().flex_wrap().gap(px(1.));
        for (index, style) in self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog["styles"].as_array())
            .into_iter()
            .flatten()
            .enumerate()
        {
            let name = style[if self.preferences.language == Language::En {
                "en"
            } else {
                "name"
            }]
            .as_str()
            .unwrap_or("")
            .to_string();
            if index == 15 {
                choices = choices.child(
                    div()
                        .w_full()
                        .h(px(1.))
                        .mx(px(6.))
                        .my(px(4.))
                        .bg(palette.edge),
                );
            }
            let preview = style["dot"]
                .as_str()
                .map(|dot| img(style_dot(dot)).w(px(18.)).h(px(18.)).into_any_element())
                .unwrap_or_else(|| div().into_any_element());
            choices = choices.child(
                choice_button(("style", index), name, index == self.style_index, Some(preview))
                    .w(px(column_width))
                    .flex_shrink_0()
                    .rounded(px(10.))
                    .when(index == self.style_index, |button| button.bg(palette.selected))
                    .disabled(
                        self.focused
                            && self.current_tile().is_some_and(|tile| {
                                !style["pieces"].as_array().is_some_and(|pieces| {
                                    pieces.iter().any(|piece| piece["id"] == tile.spec.piece)
                                })
                            }),
                    )
                    .on_click(cx.listener(move |view, _, _, cx| view.choose_style(index, cx))),
            );
        }
        body.child(choices)
    }

    fn render_layout_picker(&self, body: Div, cx: &mut Context<Self>) -> Div {
        let palette = Palette::of(cx);
        let mut choices = div().flex().flex_col().gap(px(2.));
        let selected = self
            .current_tile()
            .and_then(|tile| tile.spec.variant.as_deref());
        for (index, variant) in self
            .selected_piece()
            .and_then(|piece| piece["variants"].as_array())
            .into_iter()
            .flatten()
            .enumerate()
        {
            let id = variant["id"].as_str().unwrap_or("").to_owned();
            let name = variant[if self.preferences.language == Language::En {
                "en"
            } else {
                "name"
            }]
            .as_str()
            .unwrap_or(&id)
            .to_owned();
            choices = choices.child(
                choice_button(("variant", index), name, selected == Some(id.as_str()), None)
                    .w_full()
                    .rounded(px(9.))
                    .when(selected == Some(id.as_str()), |button| button.bg(palette.selected))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.choose_variant(id.clone(), cx);
                        view.close_panel(window, cx);
                    })),
            );
        }
        body.child(choices)
    }

    fn render_piece_picker(&self, body: Div, cx: &mut Context<Self>) -> Div {
        let mut choices = div().flex().flex_wrap().gap_2();
        for (index, piece) in self
            .style()
            .and_then(|s| s["pieces"].as_array())
            .into_iter()
            .flatten()
            .enumerate()
        {
            let id = piece["id"].as_str().unwrap_or("").to_string();
            let name = piece[if self.preferences.language == Language::En {
                "en"
            } else {
                "name"
            }]
            .as_str()
            .unwrap_or(&id)
            .to_string();
            choices = choices.child(
                Button::new(("piece", index))
                    .ghost()
                    .shell_label(name)
                    .on_click(cx.listener(move |view, _, _, cx| view.select_piece(id.clone(), cx))),
            );
        }
        body.child(choices)
    }

    fn render_blame_panel(&self, body: Div, muted: Hsla, cx: &mut Context<Self>) -> Div {
        body.child(div().text_size(px(12.)).text_color(muted).child(self.text(
            "输入仓库中的相对路径与行号。",
            "Enter a repository-relative file path and line number.",
        )))
        .child(Input::new(&self.blame_file))
        .child(Input::new(&self.blame_line))
        .child(
            Button::new("load-blame")
                .shell_label(self.text("追溯这一行", "Trace this line"))
                .disabled(self.operation_busy)
                .on_click(cx.listener(|view, _, _, cx| view.load_blame(cx))),
        )
    }

    fn render_details_panel(&self, mut body: Div, muted: Hsla, cx: &mut Context<Self>) -> Div {
        for warning in &self.warnings {
            body = body.child(div().text_size(px(12.)).child(warning.clone()));
        }
        if let Some(raw) = &self.raw {
            body = body.child(div().text_size(px(12.)).text_color(muted).child(format!(
                "{} {}",
                self.text("档案生成于", "Archive generated"),
                raw["generatedAt"].as_str().unwrap_or("—")
            )));
            for row in crate::contributions(raw) {
                let name = if self.preferences.named {
                    row.name
                } else {
                    self.text("贡献者", "Contributor").to_string()
                };
                body = body.child(
                    div()
                        .flex()
                        .justify_between()
                        .child(name)
                        .child(format!("{} / {}", row.lines_added, row.verified_lines)),
                );
            }
            body = body.child(
                Button::new("copy-details")
                    .shell_label(self.text("复制完整档案", "Copy complete archive"))
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.copy_archive(&CopyArchive, window, cx)
                    })),
            );
        }
        body
    }

    fn render_settings_navigation(
        &self,
        title: &'static str,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut navigation = div()
            .w(px(172.))
            .flex_shrink_0()
            .h_full()
            .px(px(10.))
            .py(px(18.))
            .border_r_1()
            .border_color(palette.edge)
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .px(px(10.))
                    .pb(px(12.))
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            );
        for (id, icon, name) in [
            ("general", "gear", self.text("通用", "General")),
            ("data", "folder", self.text("数据", "Data")),
            ("about", "info", self.text("关于", "About")),
        ] {
            navigation = navigation.child(
                Button::new(SharedString::from(format!("settings-tab-{id}")))
                    .ghost()
                    .h(px(34.))
                    .px(px(10.))
                    .rounded(px(10.))
                    .text_color(if self.settings_tab == id {
                        palette.ink
                    } else {
                        palette.soft
                    })
                    .when(self.settings_tab == id, |b| b.bg(palette.selected))
                    .toggled(self.settings_tab == id)
                    .accessibility_label(name)
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .text_size(px(13.))
                            .font_weight(if self.settings_tab == id {
                                FontWeight::MEDIUM
                            } else {
                                FontWeight::NORMAL
                            })
                            .child(crate::ui_icons::icon(icon))
                            .child(name),
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.settings_tab = id;
                        if id == "data" && view.report.is_none() {
                            view.request_diagnostics(cx);
                        }
                        cx.notify();
                    })),
            );
        }
        navigation
    }
}
