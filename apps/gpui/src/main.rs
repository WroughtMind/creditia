#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod renderer;

use creditia_desktop_service as service;
use creditia_gpui::{Appearance, Language, Preferences, Project, contributions, export_filename, export_snapshot, progress_text};
use gpui_kit::component::{ActiveTheme, Disableable, Theme, ThemeMode};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;

actions!(creditia, [Quit, OpenProject, Refresh, Analyze, CancelAnalysis, CancelCurrent, CopyArchive, RevealProject, ToggleLanguage, CycleAppearance, ToggleMotion, ExportArtwork, CancelExport]);

#[derive(Clone, Copy)]
enum ExportStage { ChoosePath, Rendering, Writing }

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
    analysis_cancellation: Option<service::AnalysisCancellation>,
    list_generation: u64,
    archive_generation: u64,
    list_task: Option<Task<()>>,
    archive_task: Option<Task<()>>,
    dialog_task: Option<Task<()>>,
    job_task: Option<Task<()>>,
    job_worker: Option<Task<()>>,
    catalog: Option<Value>,
    style_index: usize,
    piece_index: usize,
    variant_index: usize,
    preview: Option<(Arc<Image>, f32, f32)>,
    preview_busy: bool,
    preview_generation: u64,
    preview_task: Option<Task<()>>,
    preview_cancellation: Option<renderer::Cancellation>,
    export_format: String,
    export_size: String,
    export_label: String,
    export_stage: Option<ExportStage>,
    export_cancel_requested: bool,
    export_task: Option<Task<()>>,
    export_cancellation: Option<renderer::Cancellation>,
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
            job_busy: false, cancelling: false, analysis_cancellation: None,
            list_generation: 0, archive_generation: 0,
            list_task: None, archive_task: None, dialog_task: None,
            job_task: None, job_worker: None,
            catalog: None, style_index: 0, piece_index: 0, variant_index: 0,
            preview: None, preview_busy: false, preview_generation: 0,
            preview_task: None, preview_cancellation: None,
            export_format: "png".into(), export_size: "native".into(), export_label: String::new(), export_stage: None,
            export_cancel_requested: false, export_task: None, export_cancellation: None,
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
                            else {
                                view.archive_generation = view.archive_generation.wrapping_add(1);
                                view.archive_task = None;
                                view.clear_preview(cx);
                                view.selected = None;
                                view.raw = None;
                            }
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
        self.clear_preview(cx);
        self.archive_generation = self.archive_generation.wrapping_add(1);
        let generation = self.archive_generation;
        self.selected = Some(id.clone());
        self.preferences.last_project = Some(id.clone());
        self.error = None;
        self.persist();
        self.raw = None;
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
        let cancellation = service::AnalysisCancellation::default();
        self.analysis_cancellation = Some(cancellation.clone());
        // Progress is bounded and disposable; completion is always delivered.
        let (sender, receiver) = async_channel::bounded(32);
        let progress_sender = sender.clone();
        let sink: service::ProgressSink = Arc::new(move |value| {
            let _ = progress_sender.try_send(JobEvent::Progress(value));
        });
        self.job_worker = Some(cx.background_spawn(async move {
            let result = match add_path {
                Some(path) => service::add_project_with_cancel(path, Some(lang), Some(sink), cancellation),
                None => service::analyze_with_cancel(selected.expect("selected project"), Some(lang), Some(sink), cancellation),
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
                            view.analysis_cancellation = None;
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
            if let Some(cancellation) = &self.analysis_cancellation { cancellation.cancel(); }
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
        self.clear_preview(cx);
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
        self.notice = Some(if self.preferences.reduced_motion {
            self.text("已减少动态效果", "Reduced motion enabled")
        } else { self.text("已恢复动态效果", "Reduced motion disabled") }.into());
        cx.notify();
    }

    fn clear_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(cancellation) = self.preview_cancellation.take() { cancellation.cancel(); }
        self.preview_generation = self.preview_generation.wrapping_add(1);
        self.preview_task = None;
        self.preview_busy = false;
        if let Some((image, _, _)) = self.preview.take() { ImageSource::Image(image).remove_asset(cx); }
    }

    fn preview_artwork(&mut self, cx: &mut Context<Self>) {
        let Some(raw) = self.raw.clone() else { return; };
        self.clear_preview(cx);
        self.notice = None;
        let generation = self.preview_generation;
        let cancellation = renderer::Cancellation::default();
        self.preview_cancellation = Some(cancellation.clone());
        self.preview_busy = true;
        self.error = None;
        let cached_catalog = self.catalog.clone();
        let (style_index, piece_index, variant_index) = (self.style_index, self.piece_index, self.variant_index);
        let language = self.preferences.language.code().to_string();
        self.preview_task = Some(cx.spawn(async move |this, cx| {
            let result: Result<(Value, usize, Value), String> = cx.background_spawn(async move {
                let first_catalog = cached_catalog.is_none();
                let mut catalog = match cached_catalog {
                    Some(catalog) => catalog,
                    None => renderer::request(json!({"version":1,"id":1,"op":"catalog"}), cancellation.clone())?,
                };
                // Context-only pieces need project collections or blame inputs;
                // the single archive preview does not claim those are present.
                if let Some(styles) = catalog["styles"].as_array_mut() {
                    for style in styles {
                        if let Some(pieces) = style["pieces"].as_array_mut() {
                            pieces.retain(|piece| piece["requires"].as_array().is_none_or(Vec::is_empty));
                        }
                    }
                }
                let styles = catalog["styles"].as_array().ok_or("Renderer catalog lacks styles")?;
                let index = if first_catalog {
                    styles.iter().position(|style| style["id"] == "minimal-light").unwrap_or(0)
                } else { style_index };
                let style = styles.get(index).ok_or("Renderer style is unavailable")?;
                let piece = style["pieces"].as_array().and_then(|pieces| pieces.get(piece_index)).ok_or("Renderer piece is unavailable")?;
                let variant = piece["variants"].as_array().and_then(|variants| variants.get(variant_index))
                    .and_then(|variant| variant["id"].as_str());
                let mut request = json!({"version":1,"id":2,"op":"render","raw":raw,
                    "style":style["id"],"piece":piece["id"],"lang":language,
                    "group":"auto","named":true,"flatten":true});
                if let Some(variant) = variant { request["variant"] = json!(variant); }
                let rendered = renderer::request(request, cancellation)?;
                Ok((catalog, index, rendered["art"].clone()))
            }).await;
            let _ = this.update(cx, |view, cx| {
                if view.preview_generation != generation { return; }
                view.preview_busy = false;
                view.preview_cancellation = None;
                match result {
                    Ok((catalog, index, art)) => {
                        view.catalog = Some(catalog);
                        view.style_index = index;
                        if art.is_null() {
                            view.notice = Some(view.text("这份档案没有该作品所需的数据", "This archive has no data for this piece").into());
                        } else if let (Some(svg), Some(w), Some(h)) = (art["svg"].as_str(), art["w"].as_f64(), art["h"].as_f64()) {
                            if w > 0.0 && h > 0.0 && w <= 16384.0 && h <= 16384.0 && w * h <= 8_000_000.0 {
                                view.preview = Some((Arc::new(Image::from_bytes(ImageFormat::Svg, svg.as_bytes().to_vec())), w as f32, h as f32));
                            } else {
                                view.error = Some(view.text("作品过大，暂不能在单图预览中显示", "This piece exceeds the single-image preview limit").into());
                            }
                        } else {
                            view.error = Some("Invalid renderer artwork response".into());
                        }
                    }
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn next_style(&mut self, cx: &mut Context<Self>) {
        if let Some(styles) = self.catalog.as_ref().and_then(|c| c["styles"].as_array()) {
            if !styles.is_empty() { self.style_index = (self.style_index + 1) % styles.len(); }
        }
        self.piece_index = 0;
        self.variant_index = 0;
        self.preview_artwork(cx);
    }

    fn next_piece(&mut self, cx: &mut Context<Self>) {
        if let Some(pieces) = self.catalog.as_ref().and_then(|c| c["styles"][self.style_index]["pieces"].as_array()) {
            if !pieces.is_empty() { self.piece_index = (self.piece_index + 1) % pieces.len(); }
        }
        self.variant_index = 0;
        self.preview_artwork(cx);
    }

    fn next_variant(&mut self, cx: &mut Context<Self>) {
        if let Some(variants) = self.catalog.as_ref().and_then(|c| c["styles"][self.style_index]["pieces"][self.piece_index]["variants"].as_array()) {
            if !variants.is_empty() { self.variant_index = (self.variant_index + 1) % variants.len(); }
        }
        self.preview_artwork(cx);
    }

    fn selected_piece(&self) -> Option<&Value> {
        self.catalog.as_ref().and_then(|catalog| catalog["styles"].as_array())
            .and_then(|styles| styles.get(self.style_index))
            .and_then(|style| style["pieces"].as_array())
            .and_then(|pieces| pieces.get(self.piece_index))
    }

    fn can_export(&self) -> bool {
        self.raw.is_some() && self.catalog.is_some() && self.export_stage.is_none()
            && (self.export_format != "gif" || self.selected_piece().is_some_and(|piece| piece["animated"] == true))
    }

    fn next_export_size(&mut self, cx: &mut Context<Self>) {
        if let Some(sizes) = self.catalog.as_ref().and_then(|catalog| catalog["sizes"].as_array()) {
            if !sizes.is_empty() {
                let next = (sizes.iter().position(|size| size["id"] == self.export_size).unwrap_or(0) + 1) % sizes.len();
                if let Some(id) = sizes[next]["id"].as_str() { self.export_size = id.into(); }
            }
        }
        cx.notify();
    }

    fn export_artwork(&mut self, _: &ExportArtwork, _: &mut Window, cx: &mut Context<Self>) {
        if !self.can_export() { return; }
        let Some(raw) = self.raw.as_ref() else { return; };
        let Some(catalog) = self.catalog.as_ref() else { return; };
        // Everything describing the artwork is owned before the modal opens.
        let snapshot = match export_snapshot(raw, catalog,
            (self.style_index, self.piece_index, self.variant_index),
            &self.preferences.language, &self.export_format, &self.export_size) {
            Ok(snapshot) => snapshot,
            Err(error) => { self.error = Some(error); cx.notify(); return; }
        };
        let directory = match service::export_dir() {
            Ok(directory) => directory,
            Err(error) => { self.error = Some(error.to_string()); cx.notify(); return; }
        };
        let filename = export_filename(&snapshot);
        self.export_label = filename.clone();
        let format = self.export_format.clone();
        let prompt = cx.prompt_for_new_path(&directory, Some(&filename));
        let cancellation = renderer::Cancellation::default();
        self.export_cancellation = Some(cancellation.clone());
        self.export_stage = Some(ExportStage::ChoosePath);
        self.export_cancel_requested = false;
        self.error = None;
        self.notice = None;
        self.export_task = Some(cx.spawn(async move |this, cx| {
            let choice = prompt.await;
            let path = match choice {
                Ok(Ok(Some(path))) => path,
                other => {
                    let _ = this.update(cx, |view, cx| {
                        view.export_stage = None;
                        view.export_cancellation = None;
                        match other {
                            Ok(Ok(None)) => {},
                            Ok(Err(error)) => view.error = Some(error.to_string()),
                            Err(error) => view.error = Some(error.to_string()),
                            Ok(Ok(Some(_))) => unreachable!(),
                        }
                        cx.notify();
                    });
                    return;
                }
            };
            let Ok(proceed) = this.update(cx, |view, cx| {
                if view.export_cancel_requested {
                    view.export_stage = None;
                    view.export_cancellation = None;
                    view.notice = Some(view.text("已取消导出", "Export cancelled").into());
                    cx.notify();
                    return false;
                }
                view.export_stage = Some(ExportStage::Rendering);
                cx.notify();
                true
            }) else { cancellation.cancel(); return; };
            if !proceed { return; }
            let render_cancellation = cancellation.clone();
            let result = cx.background_spawn(async move {
                let output = renderer::request(snapshot, render_cancellation)?;
                renderer::decode_export(&output, &format)
            }).await;
            let result = match result {
                Ok(bytes) => {
                    if this.update(cx, |view, cx| {
                        view.export_stage = Some(ExportStage::Writing);
                        cx.notify();
                    }).is_err() { cancellation.cancel(); return; }
                    let output_path = path.clone();
                    cx.background_spawn(async move { renderer::write_export(&output_path, &bytes, cancellation) }).await
                }
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |view, cx| {
                view.export_stage = None;
                view.export_cancellation = None;
                match result {
                    // A successful atomic commit remains success if cancellation
                    // happened just after the final replacement.
                    Ok(()) => view.notice = Some(format!("{} {}", view.text("已保存", "Saved"), path.display())),
                    Err(_) if view.export_cancel_requested => view.notice = Some(view.text("已取消导出", "Export cancelled").into()),
                    Err(error) => view.error = Some(error),
                }
                view.export_cancel_requested = false;
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn cancel_export(&mut self, _: &CancelExport, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(cancellation) = &self.export_cancellation {
            cancellation.cancel();
            self.export_cancel_requested = true;
            cx.notify();
        }
    }

    fn cancel_current(&mut self, _: &CancelCurrent, window: &mut Window, cx: &mut Context<Self>) {
        if self.export_stage.is_some() { self.cancel_export(&CancelExport, window, cx); }
        else { self.cancel(&CancelAnalysis, window, cx); }
    }
}

impl Drop for Creditia {
    fn drop(&mut self) {
        if let Some(cancellation) = &self.analysis_cancellation { cancellation.cancel(); }
        if let Some(cancellation) = &self.preview_cancellation { cancellation.cancel(); }
        if let Some(cancellation) = &self.export_cancellation { cancellation.cancel(); }
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
            let name_key = if self.preferences.language == Language::En { "en" } else { "name" };
            let style = self.catalog.as_ref().map(|catalog| &catalog["styles"][self.style_index]);
            let piece = style.map(|style| &style["pieces"][self.piece_index]);
            let style_name = style.and_then(|style| style[name_key].as_str()).unwrap_or(self.text("风格", "Style")).to_string();
            let piece_name = piece.and_then(|piece| piece[name_key].as_str()).unwrap_or(self.text("作品", "Piece")).to_string();
            let variant_name = piece.and_then(|piece| piece["variants"][self.variant_index][name_key].as_str()).unwrap_or(self.text("版式", "Layout")).to_string();
            content = content.child(div().flex().flex_wrap().gap_2()
                .child(Button::new("preview").label(self.text("预览作品", "Preview artwork")).disabled(self.preview_busy)
                    .on_click(cx.listener(|view, _, _, cx| view.preview_artwork(cx))))
                .child(Button::new("next-style").ghost().label(style_name).disabled(self.catalog.is_none())
                    .on_click(cx.listener(|view, _, _, cx| view.next_style(cx))))
                .child(Button::new("next-piece").ghost().label(piece_name).disabled(self.catalog.is_none())
                    .on_click(cx.listener(|view, _, _, cx| view.next_piece(cx))))
                .child(Button::new("next-variant").ghost().label(variant_name).disabled(self.catalog.is_none())
                    .on_click(cx.listener(|view, _, _, cx| view.next_variant(cx)))));
            if self.preview_busy {
                content = content.child(self.text("正在生成作品…", "Rendering artwork…"));
            } else if let Some((image, width, height)) = &self.preview {
                content = content.child(img(image.clone()).id("artwork-preview").w_full().h(px((height / width * 780.).min(680.)))
                    .object_fit(ObjectFit::Contain)
                    .with_loading(|| div().child("…").into_any_element())
                    .with_fallback(|| div().child("Unable to decode artwork").into_any_element()));
            }
            if let Some(catalog) = &self.catalog {
                let animated = piece.is_some_and(|piece| piece["animated"] == true);
                let format_buttons: Vec<_> = catalog["formats"].as_array().into_iter().flatten()
                    .filter_map(|item| item["id"].as_str())
                    .filter(|id| ["png", "svg", "pdf", "gif", "html"].contains(id))
                    .enumerate().map(|(index, id)| {
                        let id = id.to_string();
                        let selected = self.export_format == id;
                        Button::new(("export-format", index)).ghost().label(id.to_uppercase())
                            .when(selected, |button| button.bg(selected_bg))
                            .disabled(self.export_stage.is_some() || (id == "gif" && !animated))
                            .on_click(cx.listener(move |view, _, _, cx| { view.export_format = id.clone(); cx.notify(); }))
                    }).collect();
                let size = catalog["sizes"].as_array().and_then(|sizes| sizes.iter().find(|size| size["id"] == self.export_size));
                let size_name = if self.export_size == "native" {
                    self.text("原尺寸", "Native size").to_string()
                } else if self.preferences.language == Language::Zh {
                    size.and_then(|size| size["name"].as_str()).unwrap_or(&self.export_size).to_string()
                } else {
                    size.map(|size| format!("{} {}×{}", self.export_size,
                        size["w"].as_u64().unwrap_or(0), size["h"].as_u64().unwrap_or(0)))
                        .unwrap_or_else(|| self.export_size.clone())
                };
                content = content.child(div().flex().flex_wrap().items_center().gap_2()
                    .children(format_buttons)
                    .child(Button::new("export-size").ghost().label(size_name)
                        .disabled(self.export_stage.is_some())
                        .on_click(cx.listener(|view, _, _, cx| view.next_export_size(cx))))
                    .child(Button::new("export-artwork").primary().label(self.text("导出…", "Export…"))
                        .disabled(!self.can_export())
                        .on_click(cx.listener(|view, _, window, cx| view.export_artwork(&ExportArtwork, window, cx)))));
                if self.export_format == "gif" {
                    content = content.child(div().text_sm().text_color(muted)
                        .child(self.text("GIF 按所选画幅缩放，最大宽度 800 px", "GIF uses the selected aspect ratio, up to 800 px wide")));
                }
            }
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
        if let Some(stage) = self.export_stage {
            let text = if self.export_cancel_requested { self.text("正在取消导出…", "Cancelling export…") }
                else { match stage {
                    ExportStage::ChoosePath => self.text("选择保存位置…", "Choose a save location…"),
                    ExportStage::Rendering => self.text("正在生成导出文件…", "Generating export…"),
                    ExportStage::Writing => self.text("正在保存文件…", "Saving file…"),
                }};
            status = status.child(format!("{text} · {}", self.export_label)).child(Button::new("cancel-export").ghost().label(self.text("取消导出", "Cancel export"))
                .disabled(self.export_cancel_requested)
                .on_click(cx.listener(|view, _, window, cx| view.cancel_export(&CancelExport, window, cx))));
        } else if self.job_busy {
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
            .on_action(cx.listener(Self::export_artwork)).on_action(cx.listener(Self::cancel_export))
            .on_action(cx.listener(Self::cancel_current))
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
            MenuItem::separator(),
            MenuItem::action(language.text("导出作品…", "Export artwork…"), ExportArtwork),
            MenuItem::action(language.text("取消导出", "Cancel export"), CancelExport),
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
            KeyBinding::new("escape", CancelCurrent, Some("Creditia")),
            KeyBinding::new("secondary-shift-c", CopyArchive, Some("Creditia")),
            KeyBinding::new("secondary-e", ExportArtwork, Some("Creditia")),
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
