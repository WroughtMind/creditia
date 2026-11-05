#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use creditia_desktop_service as service;
use creditia_gpui::{Appearance, Language, Preferences, Project, contributions, progress_text};
use gpui_kit::component::{ActiveTheme, Disableable, Theme, ThemeMode};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

actions!(creditia, [Quit, OpenProject, Refresh, Analyze, CancelAnalysis, CopyArchive, RevealProject, ToggleLanguage, CycleAppearance, ToggleMotion]);

enum JobEvent {
    Progress(Value),
    Finished(Result<Value, service::Fail>),
}

struct Creditia {
    focus: FocusHandle,
    preferences: Preferences,
    projects: Vec<Project>,
    selected: Option<String>,
    raw: Option<Value>,
    loading: bool,
    error: Option<String>,
    notice: Option<String>,
    progress: Option<Value>,
    job_busy: bool,
    cancelling: bool,
    list_generation: u64,
    archive_generation: u64,
    list_task: Option<Task<()>>,
    archive_task: Option<Task<()>>,
    dialog_task: Option<Task<()>>,
    job_task: Option<Task<()>>,
    job_worker: Option<Task<()>>,
}

impl Creditia {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let preferences = Preferences::read(&service::app_settings());
        apply_appearance(&preferences.appearance, window, cx);
        install_menus(&preferences.language, cx);
        let focus = cx.focus_handle().tab_stop(true);
        focus.focus(window, cx);
        let selected = preferences.last_project.clone();
        let mut view = Self {
            focus, preferences, selected, projects: Vec::new(), raw: None,
            loading: true, error: None, notice: None, progress: None,
            job_busy: false, cancelling: false,
            list_generation: 0, archive_generation: 0,
            list_task: None, archive_task: None, dialog_task: None,
            job_task: None, job_worker: None,
        };
        view.refresh(cx);
        view
    }

    fn text(&self, zh: &'static str, en: &'static str) -> &'static str {
        self.preferences.language.text(zh, en)
    }

    fn persist(&mut self) {
        let value = serde_json::to_value(&self.preferences).expect("preferences serialize");
        if let Err(error) = service::edit_app_settings(|settings| settings["gpui"] = value) {
            self.error = Some(error.to_string());
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.list_generation = self.list_generation.wrapping_add(1);
        let generation = self.list_generation;
        self.loading = true;
        self.error = None;
        self.list_task = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async { service::projects() }).await;
            let _ = this.update(cx, |view, cx| {
                if view.list_generation != generation { return; }
                view.loading = false;
                match result {
                    Ok(value) => match serde_json::from_value::<Vec<Project>>(value) {
                        Ok(projects) => {
                            view.projects = projects;
                            let id = view.selected.clone()
                                .filter(|id| view.projects.iter().any(|p| &p.id == id))
                                .or_else(|| view.projects.first().map(|p| p.id.clone()));
                            if let Some(id) = id { view.select(id, cx); }
                            else { view.selected = None; view.raw = None; }
                        }
                        Err(error) => view.error = Some(error.to_string()),
                    },
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn select(&mut self, id: String, cx: &mut Context<Self>) {
        self.archive_generation = self.archive_generation.wrapping_add(1);
        let generation = self.archive_generation;
        self.selected = Some(id.clone());
        self.preferences.last_project = Some(id.clone());
        self.persist();
        self.raw = None;
        self.error = None;
        self.loading = true;
        self.archive_task = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { service::credits(id) }).await;
            let _ = this.update(cx, |view, cx| {
                // A slower previous project must never replace the current archive.
                if view.archive_generation != generation { return; }
                view.loading = false;
                match result {
                    Ok(raw) => view.raw = Some(raw),
                    Err(error) if error.code == "no_credits" => {},
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn open_project(&mut self, _: &OpenProject, _: &mut Window, cx: &mut Context<Self>) {
        if self.job_busy { return; }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false, directories: true, multiple: false,
            prompt: Some(self.text("选择 Git 项目", "Choose a Git project").into()),
        });
        self.dialog_task = Some(cx.spawn(async move |this, cx| {
            let choice = prompt.await;
            let _ = this.update(cx, |view, cx| {
                match choice {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            view.start_job(Some(path.to_string_lossy().into_owned()), cx);
                        }
                    }
                    Ok(Ok(None)) => {},
                    Ok(Err(error)) => view.error = Some(error.to_string()),
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
    }

    fn start_job(&mut self, add_path: Option<String>, cx: &mut Context<Self>) {
        if self.job_busy || (add_path.is_none() && self.selected.is_none()) { return; }
        self.job_busy = true;
        self.cancelling = false;
        self.progress = None;
        self.error = None;
        self.notice = None;
        let selected = self.selected.clone();
        let selection_at_start = selected.clone();
        let lang = self.preferences.language.code().to_string();
        let adding = add_path.is_some();
        // Progress is bounded and disposable; completion is always delivered.
        let (sender, receiver) = async_channel::bounded(32);
        let progress_sender = sender.clone();
        let sink: service::ProgressSink = Arc::new(move |value| {
            let _ = progress_sender.try_send(JobEvent::Progress(value));
        });
        self.job_worker = Some(cx.background_spawn(async move {
            let result = match add_path {
                Some(path) => service::add_project(path, Some(lang), Some(sink)),
                None => service::analyze(selected.expect("selected project"), Some(lang), Some(sink)),
            };
            let _ = sender.send(JobEvent::Finished(result)).await;
        }));
        self.job_task = Some(cx.spawn(async move |this, cx| {
            while let Ok(event) = receiver.recv().await {
                let finished = matches!(&event, JobEvent::Finished(_));
                if this.update(cx, |view, cx| {
                    match event {
                        JobEvent::Progress(value) => view.progress = Some(value),
                        JobEvent::Finished(result) => {
                            view.job_busy = false;
                            view.progress = None;
                            match result {
                                Ok(value) => {
                                    if adding && view.selected == selection_at_start {
                                        view.selected = value["id"].as_str().map(str::to_string);
                                    }
                                    view.notice = Some(view.text("分析完成", "Analysis complete").into());
                                    view.refresh(cx);
                                }
                                Err(error) if view.cancelling || error.code == "cancelled" => {
                                    view.notice = Some(view.text("已取消分析", "Analysis cancelled").into());
                                }
                                Err(error) => view.error = Some(error.to_string()),
                            }
                            view.cancelling = false;
                        }
                    }
                    cx.notify();
                }).is_err() { break; }
                if finished { break; }
            }
        }));
        cx.notify();
    }

    fn analyze(&mut self, _: &Analyze, _: &mut Window, cx: &mut Context<Self>) {
        self.start_job(None, cx);
    }

    fn cancel(&mut self, _: &CancelAnalysis, _: &mut Window, cx: &mut Context<Self>) {
        if self.job_busy {
            service::cancel_analyze();
            self.cancelling = true;
            cx.notify();
        }
    }

    fn copy_archive(&mut self, _: &CopyArchive, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(raw) = &self.raw {
            match serde_json::to_string_pretty(raw) {
                Ok(text) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    self.notice = Some(self.text("档案已复制", "Archive copied").into());
                }
                Err(error) => self.error = Some(error.to_string()),
            }
            cx.notify();
        }
    }

    fn reveal_project(&mut self, _: &RevealProject, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(project) = self.current() { cx.reveal_path(Path::new(&project.path)); }
    }

    fn current(&self) -> Option<&Project> {
        self.projects.iter().find(|p| Some(&p.id) == self.selected.as_ref())
    }

    fn toggle_language(&mut self, _: &ToggleLanguage, _: &mut Window, cx: &mut Context<Self>) {
        self.preferences.language = match self.preferences.language { Language::Zh => Language::En, Language::En => Language::Zh };
        self.persist();
        install_menus(&self.preferences.language, cx);
        cx.notify();
    }

    fn cycle_appearance(&mut self, _: &CycleAppearance, window: &mut Window, cx: &mut Context<Self>) {
        self.preferences.appearance = match self.preferences.appearance {
            Appearance::System => Appearance::Light, Appearance::Light => Appearance::Dark, Appearance::Dark => Appearance::System,
        };
        apply_appearance(&self.preferences.appearance, window, cx);
        self.persist();
        cx.notify();
    }

    fn toggle_motion(&mut self, _: &ToggleMotion, _: &mut Window, cx: &mut Context<Self>) {
        self.preferences.reduced_motion = !self.preferences.reduced_motion;
        self.persist();
        cx.notify();
    }
}

impl Drop for Creditia {
    fn drop(&mut self) {
        if self.job_busy { service::cancel_analyze(); }
    }
}

impl Render for Creditia {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let background = cx.theme().background;
        let foreground = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let selected_bg = cx.theme().muted;
        let project_rows: Vec<_> = self.projects.iter().enumerate().map(|(index, project)| {
            let id = project.id.clone();
            let selected = self.selected.as_deref() == Some(&project.id);
            Button::new(("project", index)).ghost().w_full().label(project.name.clone())
                .when(selected, |button| button.bg(selected_bg))
                .on_click(cx.listener(move |view, _, _, cx| view.select(id.clone(), cx)))
        }).collect();
        let mut content = div().flex().flex_col().gap_6().p_8();
        if let Some(project) = self.current() {
            content = content.child(div().text_2xl().child(project.name.clone()))
                .child(div().text_sm().text_color(muted).child(project.path.clone()));
            if !project.exists {
                content = content.child(self.text("项目目录暂不可用，仍可读取保存的档案。", "Project folder unavailable. Saved archives remain readable."));
            }
        }
        if self.loading {
            content = content.child(self.text("正在读取作品库…", "Loading library…"));
        } else if let Some(raw) = &self.raw {
            let totals = &raw["totals"];
            let stats = [
                (self.text("新增代码行", "Lines added"), "linesAdded"),
                (self.text("存留代码行", "Surviving lines"), "linesSurviving"),
                (self.text("提交", "Commits"), "commits"),
                (self.text("会话", "Sessions"), "sessions"),
            ];
            content = content.child(div().flex().gap_8().children(stats.into_iter().map(|(label, key)| {
                div().flex().flex_col().gap_1()
                    .child(div().text_2xl().child(totals[key].as_u64().map(|v| v.to_string()).unwrap_or_else(|| "—".into())))
                    .child(div().text_sm().text_color(muted).child(label))
            })));
            content = content.child(div().flex().justify_between().border_b_1().border_color(border).pb_2()
                .child(self.text("贡献档案", "Contributions"))
                .child(div().text_sm().text_color(muted).child(self.text("新增 / 已验证", "Added / verified"))));
            for row in contributions(raw) {
                let label = if let Some(model) = row.model { format!("{} · {model}", row.name) } else { row.name };
                content = content.child(div().flex().justify_between().gap_4()
                    .child(label).child(format!("{} / {}", row.lines_added, row.verified_lines)));
            }
            content = content.child(div().text_sm().text_color(muted)
                .child(format!("{} {}", self.text("生成于", "Generated"), raw["generatedAt"].as_str().unwrap_or("—"))));
        } else if self.selected.is_some() {
            content = content.child(self.text("尚无分析档案", "No analysis archive yet"))
                .child(Button::new("first-analysis").primary().label(self.text("分析项目", "Analyze project"))
                    .disabled(self.job_busy)
                    .on_click(cx.listener(|view, _, _, cx| view.start_job(None, cx))));
        } else {
            content = content.child(div().text_2xl().child(self.text("项目作品库", "Project library")))
                .child(self.text("选择一个 Git 项目，读取它的代码贡献与证据。", "Choose a Git project to read its code contributions and evidence."))
                .child(Button::new("first-project").primary().label(self.text("打开项目…", "Open project…"))
                    .on_click(cx.listener(|view, _, window, cx| view.open_project(&OpenProject, window, cx))));
        }
        let appearance = match self.preferences.appearance {
            Appearance::System => self.text("跟随系统", "System"),
            Appearance::Light => self.text("浅色", "Light"),
            Appearance::Dark => self.text("深色", "Dark"),
        };
        let sidebar = div().flex().flex_col().w(px(244.)).flex_shrink_0().border_r_1().border_color(border).p_4().gap_4()
            .child(div().text_xl().child("creditia"))
            .child(Button::new("open").label(self.text("打开项目…", "Open project…")).disabled(self.job_busy)
                .on_click(cx.listener(|view, _, window, cx| view.open_project(&OpenProject, window, cx))))
            .child(div().id("project-list").flex_1().min_h_0().overflow_y_scroll().children(project_rows))
            .child(Button::new("language").ghost().label(self.text("English", "中文"))
                .on_click(cx.listener(|view, _, window, cx| view.toggle_language(&ToggleLanguage, window, cx))))
            .child(Button::new("appearance").ghost().label(appearance)
                .on_click(cx.listener(|view, _, window, cx| view.cycle_appearance(&CycleAppearance, window, cx))));
        let toolbar = div().flex().gap_2().p_4().border_b_1().border_color(border)
            .child(Button::new("refresh").ghost().label(self.text("刷新", "Refresh"))
                .on_click(cx.listener(|view, _, _, cx| view.refresh(cx))))
            .child(Button::new("analyze").label(self.text("重新分析", "Analyze"))
                .disabled(self.job_busy || self.selected.is_none())
                .on_click(cx.listener(|view, _, _, cx| view.start_job(None, cx))))
            .child(Button::new("copy").ghost().label(self.text("复制档案", "Copy archive")).disabled(self.raw.is_none())
                .on_click(cx.listener(|view, _, window, cx| view.copy_archive(&CopyArchive, window, cx))))
            .child(Button::new("reveal").ghost().label(self.text("打开所在位置", "Reveal folder")).disabled(self.selected.is_none())
                .on_click(cx.listener(|view, _, window, cx| view.reveal_project(&RevealProject, window, cx))));
        let mut status = div().flex().items_center().justify_between().gap_4().p_4().border_t_1().border_color(border).text_sm();
        if self.job_busy {
            let text = if self.cancelling { self.text("正在取消…", "Cancelling…").into() }
                else { self.progress.as_ref().map(|p| progress_text(p, &self.preferences.language))
                    .unwrap_or_else(|| self.text("正在分析…", "Analyzing…").into()) };
            status = status.child(text).child(Button::new("cancel").ghost().label(self.text("取消", "Cancel"))
                .disabled(self.cancelling).on_click(cx.listener(|view, _, window, cx| view.cancel(&CancelAnalysis, window, cx))));
        } else if let Some(error) = &self.error {
            status = status.child(error.clone());
        } else if let Some(notice) = &self.notice {
            status = status.child(notice.clone());
        } else {
            status = status.child(format!("{} {}", self.projects.len(), self.text("个项目", "projects")));
        }
        div().size_full().flex().bg(background).text_color(foreground)
            .track_focus(&self.focus).key_context("Creditia")
            .on_action(cx.listener(Self::open_project))
            .on_action(cx.listener(|view, _: &Refresh, _, cx| view.refresh(cx)))
            .on_action(cx.listener(Self::analyze)).on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::copy_archive)).on_action(cx.listener(Self::reveal_project))
            .on_action(cx.listener(Self::toggle_language)).on_action(cx.listener(Self::cycle_appearance))
            .on_action(cx.listener(Self::toggle_motion))
            .child(sidebar)
            .child(div().flex().flex_col().flex_1().min_w_0().child(toolbar)
                .child(div().id("archive").flex_1().min_h_0().overflow_y_scroll().child(content))
                .child(status))
    }
}

fn apply_appearance(appearance: &Appearance, window: &mut Window, cx: &mut App) {
    match appearance {
        Appearance::System => Theme::sync_system_appearance(Some(window), cx),
        Appearance::Light => Theme::change(ThemeMode::Light, Some(window), cx),
        Appearance::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
    }
}

fn install_menus(language: &Language, cx: &mut App) {
    cx.set_menus(vec![
        Menu { name: "Creditia".into(), disabled: false, items: vec![
            MenuItem::action(language.text("中英文切换", "Switch language"), ToggleLanguage),
            MenuItem::action(language.text("切换明暗主题", "Cycle appearance"), CycleAppearance),
            MenuItem::action(language.text("切换减少动态效果", "Toggle reduced motion"), ToggleMotion),
            MenuItem::separator(),
            MenuItem::action(language.text("退出 Creditia", "Quit Creditia"), Quit),
        ]},
        Menu { name: language.text("项目", "Project").into(), disabled: false, items: vec![
            MenuItem::action(language.text("打开项目…", "Open project…"), OpenProject),
            MenuItem::action(language.text("刷新", "Refresh"), Refresh),
            MenuItem::action(language.text("分析", "Analyze"), Analyze),
            MenuItem::action(language.text("取消分析", "Cancel analysis"), CancelAnalysis),
            MenuItem::separator(),
            MenuItem::action(language.text("复制档案", "Copy archive"), CopyArchive),
            MenuItem::action(language.text("打开所在位置", "Reveal folder"), RevealProject),
        ]},
    ]);
}

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        cx.on_action(|_: &Quit, cx| { service::cancel_analyze(); cx.quit(); });
        cx.bind_keys([
            KeyBinding::new("secondary-q", Quit, None),
            KeyBinding::new("secondary-o", OpenProject, Some("Creditia")),
            KeyBinding::new("secondary-r", Refresh, Some("Creditia")),
            KeyBinding::new("secondary-enter", Analyze, Some("Creditia")),
            KeyBinding::new("escape", CancelAnalysis, Some("Creditia")),
            KeyBinding::new("secondary-shift-c", CopyArchive, Some("Creditia")),
        ]);
        open_window(WindowOptions {
            titlebar: Some(TitlebarOptions { title: Some("creditia".into()), ..Default::default() }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1360.), px(880.)), cx))),
            window_min_size: Some(size(px(760.), px(560.))),
            ..Default::default()
        }, cx, |window, cx| cx.new(|cx| Creditia::new(window, cx)))
            .expect("Unable to open Creditia");
        cx.activate(true);
    });
}
