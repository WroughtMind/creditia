use super::chrome::project_date;
use super::{CopyArchive, Creditia, Panel, RevealProject};
use crate::material::{Palette, ShellLabel};
use crate::ui_controls::{icon_button, row_button, toggle_button};
use gpui_kit::TestSupportExt as _;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme, Disableable, Selectable};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde_json::Value;
use std::path::Path;

impl Creditia {
    pub(super) fn render_project_library(
        &self,
        body: Div,
        muted: Hsla,
        cx: &mut Context<Self>,
    ) -> Div {
        let items = self.picker_items(cx);
        let rows = self.render_library_projects(&items, muted, cx);
        let rows = self.render_discovery_rows(rows, &items, muted, cx);
        let rows = self.render_project_batches(rows, muted, cx);
        body.child(rows)
    }

    fn render_library_projects(
        &self,
        items: &[String],
        muted: Hsla,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let mut rows = div()
            .id("project-list")
            .track_scroll(&self.project_scroll)
            .overflow_y_scroll()
            .max_h(px((self.viewport_size.1 + 60.) * 0.52))
            .flex()
            .flex_col()
            .gap_1()
            .on_scroll(cx.listener(|view, _, _, cx| {
                view.project_row_menu = None;
                view.fill_project_marks(cx);
                cx.notify();
            }));
        if items.iter().any(|id| id == "__all__") {
            rows = rows.child(
                row_button(
                    "all-projects",
                    format!(
                        "{} · {}",
                        self.text("全部项目", "All projects"),
                        self.projects.len()
                    ),
                    "folder",
                )
                .toggled(self.selected.as_deref() == Some("__all__"))
                .when(
                    self.selected.as_deref() == Some("__all__")
                        || self.picker_selected.as_deref() == Some("__all__"),
                    |b| b.bg(Palette::of(cx).selected),
                )
                .on_click(cx.listener(|view, _, _, cx| {
                    view.panel = None;
                    view.show_all_projects(cx)
                })),
            );
        }
        for (index, project) in self
            .projects
            .iter()
            .enumerate()
            .filter(|(_, p)| items.contains(&p.id))
        {
            let id = project.id.clone();
            let more_id = id.clone();
            let selected = self.selected.as_ref() == Some(&id);
            let mark = self.project_marks.get(&id).map_or_else(
                || crate::ui_icons::icon("folder").into_any_element(),
                |mark| {
                    img(mark.image.clone())
                        .w(px(20.))
                        .h(px(20.))
                        .into_any_element()
                },
            );
            let date = if !project.exists {
                self.text("目录不在了", "Folder missing").to_owned()
            } else {
                project
                    .last_analyzed
                    .map(|stamp| project_date(stamp, &self.preferences.language))
                    .unwrap_or_default()
            };
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .min_w_0()
                    .gap_1()
                    .rounded(px(14.))
                    .border_1()
                    .border_color(gpui_kit::transparent_black())
                    .when(
                        selected || self.picker_selected.as_ref() == Some(&id),
                        |row| row.bg(Palette::of(cx).selected),
                    )
                    .when(self.picker_selected.as_ref() == Some(&id), |row| {
                        row.border_color(Palette::of(cx).ink.opacity(0.2))
                    })
                    .child(
                        Button::new(("choose-project", index))
                            .accessibility_label(project.name.clone())
                            .toggled(selected)
                            .ghost()
                            .flex_1()
                            .min_w_0()
                            .h(px(52.))
                            .px(px(8.))
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .w(px(24.))
                                            .flex_shrink_0()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(mark),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .truncate()
                                                    .child(project.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(muted)
                                                    .truncate()
                                                    .child(crate::material::short_path(
                                                        &project.path,
                                                    )),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_size(px(11.))
                                            .text_color(muted)
                                            .child(date),
                                    ),
                            )
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.panel = None;
                                view.project_row_menu = None;
                                view.select(id.clone(), cx)
                            })),
                    )
                    .child(
                        icon_button(
                            ("project-more", index),
                            "more",
                            self.text("项目操作", "Project actions"),
                        )
                        .flex_shrink_0()
                        .on_click(cx.listener(
                            move |view, event: &ClickEvent, _, cx| {
                                if view
                                    .project_row_menu
                                    .as_ref()
                                    .is_some_and(|(id, _)| id == &more_id)
                                {
                                    view.project_row_menu = None;
                                } else {
                                    let point = event.position();
                                    let x: f32 = point.x.into();
                                    let y: f32 = point.y.into();
                                    view.project_row_menu =
                                        Some((more_id.clone(), (x - 216., y + 4.)));
                                }
                                cx.stop_propagation();
                                cx.notify();
                            },
                        )),
                    ),
            );
        }
        rows
    }

    fn render_discovery_rows(
        &self,
        mut rows: Stateful<Div>,
        items: &[String],
        muted: Hsla,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let discovered = self
            .discovered
            .as_ref()
            .into_iter()
            .flatten()
            .enumerate()
            .filter(|(_, p)| {
                p["path"]
                    .as_str()
                    .is_some_and(|path| items.contains(&format!("path:{path}")))
            })
            .collect::<Vec<_>>();
        for (index, project) in discovered {
            let path = project["path"].as_str().unwrap_or("").to_string();
            let selected_path = path.clone();
            let run_path = path.clone();
            let key = format!("path:{path}");
            let picked = self.discovery_selected.contains(&path);
            let name = project["name"].as_str().unwrap_or(&path).to_string();
            let tools = project["tools"]
                .as_object()
                .map(|tools| tools.keys().cloned().collect::<Vec<_>>().join(" · "))
                .unwrap_or_default();
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .rounded(px(14.))
                    .when(self.picker_selected.as_ref() == Some(&key), |row| {
                        row.bg(Palette::of(cx).selected)
                    })
                    .child(
                        Button::new(("discovered", index))
                            .ghost()
                            .shell_label(if picked { "✓" } else { "○" })
                            .disabled(self.job_busy)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if let Some(i) = view
                                    .discovery_selected
                                    .iter()
                                    .position(|p| p == &selected_path)
                                {
                                    view.discovery_selected.remove(i);
                                } else {
                                    view.discovery_selected.push(selected_path.clone());
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(("discover-open", index))
                            .ghost()
                            .flex_1()
                            .h(px(52.))
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .flex_col()
                                    .items_start()
                                    .gap_1()
                                    .child(
                                        div().w_full().flex().justify_between().child(name).child(
                                            div().text_size(px(11.)).text_color(muted).child(tools),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(muted)
                                            .truncate()
                                            .child(path),
                                    ),
                            )
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.queue_projects([run_path.clone()], cx)
                            })),
                    ),
            );
        }

        if self.discover_busy {
            rows = rows.child(self.text("正在查找本机项目…", "Finding local projects…"));
        }
        if let Some(error) = &self.discovery_error {
            rows = rows
                .child(div().text_size(px(12.)).child(error.clone()))
                .child(
                    Button::new("retry-discovery")
                        .ghost()
                        .shell_label(self.text("重新查找", "Try again"))
                        .on_click(cx.listener(|view, _, _, cx| view.retry_discovery(cx))),
                );
        } else if items.is_empty() && !self.discover_busy {
            rows = rows.child(self.text("没有匹配的项目", "No matching projects"));
        }
        if !self.discovery_selected.is_empty() {
            rows = rows.child(
                Button::new("add-selected")
                    .shell_label(format!(
                        "{} ({})",
                        self.text("分析所选项目", "Analyze selected"),
                        self.discovery_selected.len()
                    ))
                    .disabled(self.job_busy)
                    .on_click(cx.listener(|view, _, _, cx| {
                        let paths = std::mem::take(&mut view.discovery_selected);
                        view.queue_projects(paths, cx);
                    })),
            );
        }
        rows
    }

    fn render_project_batches(
        &self,
        mut rows: Stateful<Div>,
        muted: Hsla,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        for (index, path) in self.batch_order.iter().enumerate() {
            let Some(result) = self.batch_results.get(path) else {
                continue;
            };
            let state = result["state"].as_str().unwrap_or("queued");
            let label = match state {
                "running" => self.text("分析中", "Analyzing"),
                "done" => self.text("已完成", "Done"),
                "failed" => self.text("失败", "Failed"),
                "cancelled" => self.text("已取消", "Cancelled"),
                _ => self.text("等待中", "Queued"),
            };
            let retry = path.clone();
            rows = rows.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().text_size(px(12.)).child(format!(
                                    "{} · {}",
                                    Path::new(path)
                                        .file_name()
                                        .and_then(|p| p.to_str())
                                        .unwrap_or(path),
                                    label
                                )))
                            .when(matches!(state, "failed" | "cancelled"), |row| {
                                row.child(
                                    Button::new(("retry-batch", index))
                                        .ghost()
                                        .shell_label(self.text("重试", "Retry"))
                                        .disabled(self.job_busy)
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            view.queue_projects([retry.clone()], cx)
                                        })),
                                )
                            }),
                    )
                    .when_some(result["error"].as_str(), |row, error| {
                        row.child(
                            div()
                                .text_size(px(11.))
                                .text_color(muted)
                                .child(error.to_string()),
                        )
                    })
                    .children(
                        result["warnings"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .map(|note| {
                                div()
                                    .text_size(px(11.))
                                    .text_color(muted)
                                    .child(note.to_string())
                            }),
                    ),
            );
        }
        rows
    }

    pub(super) fn render_project_settings(
        &self,
        mut body: Div,
        muted: Hsla,
        cx: &mut Context<Self>,
    ) -> Div {
        body = body.child(
            div()
                .flex()
                .gap_2()
                .child(
                    Button::new("analyze")
                        .ghost()
                        .icon(crate::ui_icons::icon("refresh"))
                        .shell_label(self.text("重新分析", "Analyze"))
                        .disabled(
                            self.job_busy
                                || self.automatic_job.is_some()
                                || self
                                    .settings_current()
                                    .is_none_or(|project| !project.exists),
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.start_job_target(None, false, view.settings_id().cloned(), cx)
                        })),
                )
                .child(
                    Button::new("copy")
                        .ghost()
                        .icon(crate::ui_icons::icon("copy"))
                        .shell_label(self.text("复制档案", "Copy archive"))
                        .disabled(
                            self.raw.is_none() || self.settings_id() != self.selected.as_ref(),
                        )
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.copy_archive(&CopyArchive, window, cx)
                        })),
                )
                .child(
                    Button::new("details")
                        .ghost()
                        .icon(crate::ui_icons::icon("info"))
                        .shell_label(self.text("证据", "Evidence"))
                        .disabled(
                            self.raw.is_none() || self.settings_id() != self.selected.as_ref(),
                        )
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.open_panel(Panel::Details, window, cx)
                        })),
                ),
        );
        if let Some(project) = self.settings_current() {
            body = body
                .child(div().text_size(px(15.)).child(project.name.clone()))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(muted)
                        .child(crate::material::short_path(&project.path)),
                );
            if let Some(stamp) = project.last_analyzed {
                body = body.child(div().text_size(px(12.)).text_color(muted).child(format!(
                    "{} {}",
                    self.text("上次分析", "Last analyzed"),
                    project_date(stamp, &self.preferences.language)
                )));
            }
        }
        body = body
            .child(
                div()
                    .mt_2()
                    .text_size(px(12.))
                    .text_color(muted)
                    .child(self.text("作品署名（可选）", "Artwork signature (optional)")),
            )
            .child(
                div()
                    .id("project-signature")
                    .test_support()
                    .track_focus(&self.sign_input.read(cx).focus_handle(cx))
                    .child(Input::new(&self.sign_input)),
            )
            .child(
                Button::new("save-display")
                    .shell_label(
                        self.text("保存署名与姓名设置", "Save signature and name settings"),
                    )
                    .disabled(
                        self.operation_busy
                            || self.settings_data().is_none()
                            || self.sign_loaded_for.as_ref() != self.settings_id(),
                    )
                    .on_click(cx.listener(|view, _, _, cx| view.save_project_display(cx))),
            );
        let current_auto = self
            .settings_id()
            .and_then(|id| self.preferences.projects.get(id))
            .and_then(|prefs| prefs.auto_analyze);
        let mut auto = div().flex().flex_wrap().gap_1();
        for (index, (mode, label)) in [
            (None, self.text("跟随全局", "Global default")),
            (Some(true), self.text("自动分析", "Analyze automatically")),
            (Some(false), self.text("手动分析", "Analyze manually")),
        ]
        .into_iter()
        .enumerate()
        {
            auto = auto.child(
                Button::new(("project-auto", index))
                    .ghost()
                    .shell_label(label)
                    .selected(current_auto == mode)
                    .toggled(current_auto == mode)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if let Some(id) = view.settings_id().cloned() {
                            view.preferences
                                .projects
                                .entry(id)
                                .or_default()
                                .auto_analyze = mode;
                            view.persist();
                            if let Some(background) = &view.background {
                                background.refresh();
                            }
                        }
                        cx.notify();
                    })),
            );
        }
        body = body.child(auto);
        for (which, zh, en) in [
            ("hook", "提交时记录证据", "Record evidence on commit"),
            ("release", "发布时生成作品", "Generate artwork on release"),
        ] {
            let known = self
                .settings_data()
                .is_some_and(|d| d["integrations"][which].is_object());
            let exists = self
                .settings_current()
                .is_some_and(|project| project.exists);
            let on = self
                .settings_data()
                .and_then(|d| {
                    d["integrations"][which]["enabled"]
                        .as_bool()
                        .or_else(|| d["integrations"][which]["installed"].as_bool())
                })
                .unwrap_or(false);
            body = body.child(
                toggle_button(
                    which,
                    self.text(zh, en),
                    on,
                    !self.operation_busy && known && exists,
                    cx,
                )
                .on_click(cx.listener(move |view, _, _, cx| view.set_integration(which, !on, cx))),
            );
            if let Some(details) = self.settings_data() {
                if let Some(path) = details["integrations"][which]["path"].as_str() {
                    body = body.child(
                        div()
                            .text_size(px(11.))
                            .text_color(muted)
                            .child(path.to_string()),
                    );
                }
                if which == "release" && known {
                    body = body.child(div().text_size(px(11.)).text_color(muted).child(
                        if details["integrations"][which]["github"] == true {
                            self.text("GitHub 发版工作流", "GitHub release workflow")
                        } else {
                            self.text("需要 GitHub 远端仓库", "A GitHub remote is required")
                        },
                    ));
                }
            }
        }
        if let Some(details) = self.settings_data() {
            if !details["integrationsError"].is_null() {
                body = body
                    .child(
                        div().text_size(px(12.)).child(
                            crate::ui_reports::error_text(
                                &details["integrationsError"],
                                &self.preferences.language,
                            )
                            .unwrap_or_default(),
                        ),
                    )
                    .child(
                        row_button(
                            "retry-integrations",
                            self.text("重新检查接入", "Check integrations again"),
                            "refresh",
                        )
                        .on_click(cx.listener(|view, _, _, cx| view.fetch_details(cx))),
                    );
            }
            if details["integrations"]["release"]["published"] == true {
                body = body.child(div().text_size(px(12.)).text_color(muted).child(self.text(
                    "仓库中已有发布档案",
                    "Published archive exists in this repository",
                )));
            }
            if details["integrations"]["release"]["installed"] == true
                && details["integrations"]["hook"]["installed"] != true
            {
                body = body.child(div().text_size(px(12.)).text_color(muted).child(self.text(
                    "提交接入可为之后的发布补充证据",
                    "Enable commit integration to retain evidence for future releases",
                )));
            }
            body = body.child(div().text_size(px(12.)).text_color(muted).child(format!(
                "{} {} · {} {}",
                self.text("证据", "Evidence"),
                details["evidence"],
                self.text("会话", "Sessions"),
                details["sessions"].as_array().map(Vec::len).unwrap_or(0)
            )));
            for (index, path) in details["paths"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
            {
                let kind = path["kind"].as_str().unwrap_or("primary");
                let label = match kind {
                    "manual" => self.text("手动关联", "Linked"),
                    "auto" => self.text("自动发现", "Discovered"),
                    _ => self.text("主目录", "Main"),
                };
                let manual = kind == "manual";
                let path = path["path"].as_str().unwrap_or("").to_string();
                let unlink = path.clone();
                body = body.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().text_size(px(11.)).text_color(muted).child(label))
                        .child(div().flex_1().text_size(px(12.)).child(path))
                        .when(manual, |row| {
                            row.child(
                                Button::new(("unlink", index))
                                    .ghost()
                                    .shell_label(self.text("解除关联", "Unlink"))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.unlink_path(unlink.clone(), cx)
                                    })),
                            )
                        }),
                );
            }
        }
        body = body
            .child(
                row_button(
                    "evidence-export",
                    self.text("导出证据档案…", "Export evidence archive…"),
                    "export",
                )
                .disabled(self.operation_busy || self.job_busy)
                .on_click(cx.listener(|view, _, _, cx| view.export_evidence(cx))),
            )
            .child(
                row_button(
                    "evidence-import",
                    self.text("导入证据档案…", "Import evidence archive…"),
                    "folder",
                )
                .disabled(self.operation_busy || self.job_busy)
                .on_click(cx.listener(|view, _, _, cx| view.import_evidence(cx))),
            )
            .child(
                row_button(
                    "blame-query",
                    self.text("查询逐行归因…", "Trace a line…"),
                    "search",
                )
                .on_click(cx.listener(|view, _, window, cx| {
                    if let Some(id) = view.settings_id().cloned()
                        && view.selected.as_ref() != Some(&id)
                    {
                        view.select(id, cx);
                    }
                    view.open_panel(Panel::Blame, window, cx)
                })),
            );
        body = body
            .child(
                row_button(
                    "link-path",
                    self.text("关联目录…", "Link directory…"),
                    "link",
                )
                .disabled(self.operation_busy || self.settings_current().is_none_or(|p| !p.exists))
                .on_click(cx.listener(|view, _, _, cx| view.link_folder(cx))),
            )
            .child(
                row_button(
                    "reveal",
                    self.text("打开所在位置", "Reveal folder"),
                    "folder",
                )
                .disabled(self.settings_current().is_none_or(|p| !p.exists))
                .on_click(cx.listener(|view, _, window, cx| {
                    view.reveal_project(&RevealProject, window, cx)
                })),
            )
            .child(
                row_button(
                    "remove-project",
                    if self.remove_confirm {
                        self.text("确认移除，证据仍保留", "Confirm removal, keep evidence")
                    } else {
                        self.text(
                            "从作品库移除（保留证据）",
                            "Remove from library (keep evidence)",
                        )
                    },
                    "remove",
                )
                .disabled(self.operation_busy)
                .on_click(cx.listener(|view, _, _, cx| view.remove_project(cx))),
            );
        body
    }

    pub(super) fn render_project_row_menu(
        &self,
        panel: Panel,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.project_row_menu
            .as_ref()
            .filter(|_| panel == Panel::Projects)
            .and_then(|(id, position)| {
                let project = self.projects.iter().find(|p| &p.id == id)?;
                let analyze = id.clone();
                let settings = id.clone();
                let remove = id.clone();
                let path = project.path.clone();
                Some(
                    anchored()
                        .anchor(Anchor::TopLeft)
                        .position(point(px(position.0), px(position.1)))
                        .snap_to_window_with_margin(px(18.))
                        .child(
                            div()
                                .id("project-row-menu")
                                .child(self.surface_anchors.tracker("project-row-menu"))
                                .test_support()
                                .occlude()
                                .w(px(232.))
                                .p_2()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .rounded_lg()
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .child(
                                    row_button(
                                        "row-analyze",
                                        self.text("重新分析", "Re-analyze"),
                                        "refresh",
                                    )
                                    .h(px(32.))
                                    .disabled(
                                        !project.exists
                                            || self.job_busy
                                            || self.automatic_job.is_some(),
                                    )
                                    .on_click(cx.listener(
                                        move |view, _, _, cx| {
                                            view.project_row_menu = None;
                                            view.start_job_target(
                                                None,
                                                false,
                                                Some(analyze.clone()),
                                                cx,
                                            )
                                        },
                                    )),
                                )
                                .child(
                                    row_button(
                                        "row-reveal",
                                        self.text("打开所在位置", "Reveal folder"),
                                        "folder",
                                    )
                                    .h(px(32.))
                                    .disabled(!project.exists)
                                    .on_click(cx.listener(
                                        move |view, _, _, cx| {
                                            view.project_row_menu = None;
                                            cx.reveal_path(Path::new(&path));
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    row_button(
                                        "row-settings",
                                        self.text("项目设置…", "Project settings…"),
                                        "gear",
                                    )
                                    .h(px(32.))
                                    .on_click(cx.listener(
                                        move |view, _, window, cx| {
                                            view.open_project_settings(
                                                settings.clone(),
                                                false,
                                                window,
                                                cx,
                                            )
                                        },
                                    )),
                                )
                                .child(
                                    row_button(
                                        "row-remove",
                                        self.text("从作品库移除…", "Remove from library…"),
                                        "remove",
                                    )
                                    .h(px(32.))
                                    .on_click(cx.listener(
                                        move |view, _, window, cx| {
                                            view.open_project_settings(
                                                remove.clone(),
                                                true,
                                                window,
                                                cx,
                                            )
                                        },
                                    )),
                                ),
                        )
                        .into_any_element(),
                )
            })
    }
}
