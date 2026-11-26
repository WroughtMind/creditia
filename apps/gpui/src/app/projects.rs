use super::{
    Analyze, CancelAnalysis, CopyArchive, Creditia, JobEvent, OpenProject, Panel, RevealProject,
};
use crate::{Project, renderer};
use creditia_desktop_service as service;
use gpui_kit::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

impl Creditia {
    pub(super) fn persist(&mut self) {
        if let Some(writer) = &self.preference_writer {
            writer.submit(self.preferences.shared_patch());
        }
    }

    pub(super) fn project_preferences(&self) -> crate::ProjectPreferences {
        let mut preferences = self.preferences.project(self.selected.as_deref());
        if let Some(styles) = &self.focus_styles
            && styles.current != styles.original
        {
            preferences.variants = styles
                .variants
                .get(&styles.current)
                .cloned()
                .unwrap_or_default();
            preferences.order.clear();
        }
        preferences
    }

    pub(super) fn current(&self) -> Option<&Project> {
        self.projects
            .iter()
            .find(|p| Some(&p.id) == self.selected.as_ref())
    }

    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        self.list_generation = self.list_generation.wrapping_add(1);
        let generation = self.list_generation;
        self.loading = true;
        self.error = None;
        self.list_task = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async { service::projects() }).await;
            let _ = this.update(cx, |view, cx| {
                if view.list_generation != generation {
                    return;
                }
                view.loading = false;
                match result {
                    Ok(value) => match serde_json::from_value::<Vec<Project>>(value) {
                        Ok(projects) => {
                            view.projects = projects;
                            if view.projects.is_empty() {
                                view.archive_generation = view.archive_generation.wrapping_add(1);
                                view.archive_task = None;
                                view.clear_preview(cx);
                                view.selected = None;
                                view.raw = None;
                                view.preferences.last_project = None;
                                view.persist();
                                if let Some(background) = &view.background {
                                    background.watch_project(None);
                                }
                                view.discovered = None;
                                view.discover_projects(cx);
                                cx.notify();
                                return;
                            }
                            if view.selected.as_deref() == Some("__all__") {
                                view.show_all_projects(cx);
                                return;
                            }
                            let id = view
                                .selected
                                .clone()
                                .filter(|id| view.projects.iter().any(|p| &p.id == id))
                                .or_else(|| view.projects.first().map(|p| p.id.clone()));
                            if let Some(id) = id {
                                view.select(id, cx);
                            } else {
                                view.archive_generation = view.archive_generation.wrapping_add(1);
                                view.archive_task = None;
                                view.clear_preview(cx);
                                view.selected = None;
                                view.raw = None;
                                view.discover_projects(cx);
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

    pub(super) fn select(&mut self, id: String, cx: &mut Context<Self>) {
        let same_project = self.selected.as_ref() == Some(&id);
        if !same_project {
            self.focus_styles = None;
            self.drop_focus_placeholder(cx);
        }
        let refresh_projects = same_project && !self.all_archives.is_empty();
        let refresh_blame = same_project
            .then(|| {
                self.blame.as_ref().and_then(|blame| {
                    Some((blame["file"].as_str()?.to_string(), blame["line"].as_u64()?))
                })
            })
            .flatten();
        self.pending_view = if self.selected.as_ref() == Some(&id) && self.raw.is_some() {
            Some(self.bookmark())
        } else {
            None
        };
        self.clear_preview(cx);
        self.archive_generation = self.archive_generation.wrapping_add(1);
        let generation = self.archive_generation;
        self.selected = Some(id.clone());
        self.all_archives = Arc::new(vec![]);
        self.blame = None;
        self.focused = false;
        self.details = None;
        // Changing the watch cancels its worker asynchronously. Keep its identity
        // until completion so a queued project does not race the analysis lock.
        if !self.job_busy && !same_project {
            self.progress = None;
        }
        if let Some(background) = &self.background {
            background.watch_project(Some(id.clone()));
        }
        self.preferences.last_project = Some(id.clone());
        self.error = None;
        self.persist();
        self.raw = None;
        self.render_archive = None;
        self.loading = true;
        self.archive_task = Some(cx.spawn(async move |this, cx| {
            let (result, details, projects, blame) = cx
                .background_spawn(async move {
                    let projects = refresh_projects.then(service::project_archives);
                    let blame =
                        refresh_blame.map(|(file, line)| service::blame(id.clone(), file, line));
                    (
                        service::credits(id.clone()),
                        service::project_details(id),
                        projects,
                        blame,
                    )
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                // A slower previous project must never replace the current archive.
                if view.archive_generation != generation {
                    return;
                }
                view.loading = false;
                if let Some(projects) = projects {
                    match projects {
                        Ok(projects) => {
                            view.all_archives = Arc::new(
                                projects
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(|project| project.get("raw").cloned())
                                    .collect(),
                            )
                        }
                        Err(error) => {
                            view.notice = Some(crate::ui_reports::failure(
                                &error,
                                &view.preferences.language,
                            ))
                        }
                    }
                }
                if let Some(blame) = blame {
                    match blame {
                        Ok(blame) => view.blame = Some(blame),
                        Err(error) => {
                            view.notice = Some(crate::ui_reports::failure(
                                &error,
                                &view.preferences.language,
                            ))
                        }
                    }
                }
                match details {
                    Ok(details) => view.details = Some(details),
                    Err(error) => {
                        view.notice = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
                }
                match result {
                    Ok(raw) => {
                        view.raw = Some(Arc::new(raw));
                        view.load_catalog(cx);
                    }
                    Err(error) if error.code == "no_credits" => {}
                    Err(error) => {
                        view.error = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn queue_projects(
        &mut self,
        paths: impl IntoIterator<Item = String>,
        cx: &mut Context<Self>,
    ) {
        if self.active_add.is_none() && self.add_queue.is_empty() {
            self.batch_opened = false;
        }
        for path in paths {
            if self.active_add.as_ref() == Some(&path) || self.add_queue.contains(&path) {
                continue;
            }
            if !self.batch_order.contains(&path) {
                self.batch_order.push(path.clone());
            }
            self.batch_results
                .insert(path.clone(), json!({"state":"queued"}));
            self.add_queue.push_back(path);
        }
        self.start_next_project(cx);
        cx.notify();
    }

    pub(super) fn retry_discovery(&mut self, cx: &mut Context<Self>) {
        self.discovered = None;
        self.discovery_error = None;
        self.discover_projects(cx);
    }

    pub(super) fn start_next_project(&mut self, cx: &mut Context<Self>) {
        if self.job_busy || self.automatic_job.is_some() {
            return;
        }
        if let Some(path) = self.add_queue.pop_front() {
            self.start_job(Some(path), cx);
        }
    }

    pub(super) fn discover_projects(&mut self, cx: &mut Context<Self>) {
        if self.fixture || self.discovered.is_some() || self.discover_busy {
            return;
        }
        self.discover_busy = true;
        self.discovery_error = None;
        self.discovery_task = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async { service::discover() }).await;
            let _ = this.update(cx, |view, cx| {
                view.discover_busy = false;
                match result {
                    Ok(value) => {
                        let projects = value["projects"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter(|p| {
                                p["exists"] == true
                                    && !view.projects.iter().any(|known| {
                                        Some(known.path.as_str()) == p["path"].as_str()
                                    })
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        view.discovery_selected = projects
                            .iter()
                            .take(3)
                            .filter_map(|p| p["path"].as_str().map(str::to_owned))
                            .collect();
                        view.discovered = Some(projects);
                    }
                    Err(error) => {
                        view.discovery_error = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn open_project(&mut self, _: &OpenProject, _: &mut Window, cx: &mut Context<Self>) {
        if self.job_busy {
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some(self.text("选择 Git 项目", "Choose a Git project").into()),
        });
        self.dialog_task = Some(cx.spawn(async move |this, cx| {
            let choice = prompt.await;
            let _ = this.update(cx, |view, cx| {
                match choice {
                    Ok(Ok(Some(paths))) => {
                        view.queue_projects(
                            paths
                                .into_iter()
                                .map(|path| path.to_string_lossy().into_owned()),
                            cx,
                        );
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => view.error = Some(error.to_string()),
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn start_job(&mut self, add_path: Option<String>, cx: &mut Context<Self>) {
        self.start_job_mode(add_path, false, cx);
    }

    pub(super) fn start_job_mode(
        &mut self,
        add_path: Option<String>,
        resume: bool,
        cx: &mut Context<Self>,
    ) {
        self.start_job_target(add_path, resume, None, cx);
    }

    pub(super) fn start_job_target(
        &mut self,
        add_path: Option<String>,
        resume: bool,
        target: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let selected = target.or_else(|| self.selected.clone());
        if self.job_busy
            || self.automatic_job.is_some()
            || (!resume
                && add_path.is_none()
                && selected
                    .as_ref()
                    .and_then(|id| self.projects.iter().find(|project| &project.id == id))
                    .is_none_or(|project| !project.exists))
        {
            return;
        }
        self.job_busy = true;
        self.cancelling = false;
        self.progress = None;
        self.error = None;
        self.notice = None;
        if add_path.is_none() {
            self.warnings.clear();
        }
        self.active_add = add_path.clone();
        if let Some(path) = &add_path {
            self.batch_results
                .insert(path.clone(), json!({"state":"running"}));
        }
        let selection_at_start = self.selected.clone();
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
            let result = if resume {
                service::resume_analysis(Some(sink), cancellation)
            } else {
                match add_path {
                    Some(path) => {
                        service::add_project_with_cancel(path, Some(lang), Some(sink), cancellation)
                    }
                    None => service::analyze_with_cancel(
                        selected.expect("selected project"),
                        Some(lang),
                        Some(sink),
                        cancellation,
                    ),
                }
            };
            let _ = sender.send(JobEvent::Finished(result)).await;
        }));
        self.job_task = Some(cx.spawn(async move |this, cx| {
            while let Ok(event) = receiver.recv().await {
                let finished = matches!(&event, JobEvent::Finished(_));
                if this
                    .update(cx, |view, cx| {
                        match event {
                            JobEvent::Progress(value) => view.progress = Some(value),
                            JobEvent::Finished(result) => {
                                view.job_busy = false;
                                view.analysis_cancellation = None;
                                view.progress = None;
                                match result {
                                    Ok(value) => {
                                        let warnings = crate::ui_reports::analysis_warnings(&value);
                                        if adding {
                                            view.warnings.extend(warnings.clone());
                                            if let Some(path) = view.active_add.take() {
                                                view.batch_results.insert(
                                                    path.clone(),
                                                    json!({"state":"done","warnings":warnings}),
                                                );
                                                if let Some(found) = &mut view.discovered {
                                                    found.retain(|project| {
                                                        project["path"].as_str() != Some(&path)
                                                    });
                                                }
                                            }
                                        } else {
                                            view.warnings = warnings;
                                        }
                                        if adding && !view.batch_opened {
                                            if view.selected==selection_at_start {view.selected=value["id"].as_str().map(str::to_string);}
                                            view.batch_opened=true;
                                        }
                                        view.show_success(view.text("分析完成", "Analysis complete").into(), cx);
                                        view.refresh(cx);
                                    }
                                    Err(error) if view.cancelling || error.code == "cancelled" => {
                                        if let Some(path) = view.active_add.take() {
                                            view.batch_results
                                                .insert(path, json!({"state":"cancelled"}));
                                        }
                                        view.show_success(view.text("已取消分析", "Analysis cancelled").into(), cx);
                                    }
                                    Err(error) => {
                                        if let Some(path) = view.active_add.take() {
                                            view.batch_results.insert(
                                                path,
                                                json!({"state":"failed","error":crate::ui_reports::failure(&error,&view.preferences.language)}),
                                            );
                                        }
                                        view.error = Some(crate::ui_reports::failure(&error,&view.preferences.language));
                                    }
                                }
                                view.cancelling = false;
                                view.pending_analysis = service::analysis_status();
                                view.start_next_project(cx);
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
                if finished {
                    break;
                }
            }
        }));
        cx.notify();
    }

    pub(super) fn analyze(&mut self, _: &Analyze, _: &mut Window, cx: &mut Context<Self>) {
        self.start_job(None, cx);
    }

    pub(super) fn cancel(&mut self, _: &CancelAnalysis, _: &mut Window, cx: &mut Context<Self>) {
        if !self.job_busy
            && let (Some((id, generation)), Some(background)) =
                (&self.automatic_job, &self.background)
        {
            self.cancelling = background.cancel_analysis(id, *generation);
            cx.notify();
        }
        if self.job_busy {
            for path in self.add_queue.drain(..) {
                self.batch_results
                    .insert(path, json!({"state":"cancelled"}));
            }
            if let Some(cancellation) = &self.analysis_cancellation {
                cancellation.cancel();
            }
            self.cancelling = true;
            cx.notify();
        }
    }

    pub(super) fn copy_archive(&mut self, _: &CopyArchive, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(raw) = &self.raw {
            match serde_json::to_string_pretty(raw.as_ref()) {
                Ok(text) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    self.show_success(self.text("档案已复制", "Archive copied").into(), cx);
                }
                Err(error) => self.error = Some(error.to_string()),
            }
            cx.notify();
        }
    }

    pub(super) fn reveal_project(
        &mut self,
        _: &RevealProject,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let project = if self.panel == Some(Panel::Project) {
            self.settings_current()
        } else {
            self.current()
        };
        if let Some(project) = project.filter(|project| project.exists) {
            cx.reveal_path(Path::new(&project.path));
        }
    }

    pub(super) fn update_project_mark(&mut self, cx: &mut Context<Self>) {
        if self.fixture && !self.benchmark {
            return;
        }
        let Some(raw) = self.raw.clone() else {
            if self.mark_source.take().is_some() {
                self.mark_generation = self.mark_generation.wrapping_add(1);
                if let Some(cancel) = self.mark_cancellation.take() {
                    cancel.cancel();
                }
                let mark = self
                    .selected
                    .as_ref()
                    .and_then(|id| self.project_marks.get(id))
                    .cloned();
                self.live_mark.set_project(mark.clone(), cx);
                self.center_mark.set_project(mark, cx);
            }
            return;
        };
        let group = self.group().to_string();
        let lang = self.preferences.language.code().to_string();
        let named = self.preferences.named;
        let source = (
            Arc::as_ptr(&raw) as usize,
            group.clone(),
            named,
            lang.clone(),
        );
        if self.mark_source.as_ref() == Some(&source) {
            return;
        }
        self.mark_source = Some(source);
        self.mark_generation = self.mark_generation.wrapping_add(1);
        let generation = self.mark_generation;
        if let Some(cancel) = self.mark_cancellation.take() {
            cancel.cancel();
        }
        let cancellation = renderer::Cancellation::default();
        self.mark_cancellation = Some(cancellation.clone());
        let id = self.selected.clone();
        self.mark_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let request = json!({
                        "version":1, "id":"project-mark", "op":"mark", "raw":raw.as_ref(),
                        "group":group, "lang":lang, "named":named
                    });
                    renderer::request_with_priority(request, cancellation, 0)
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.mark_generation != generation {
                    return;
                }
                view.mark_cancellation = None;
                if let Ok(value) = &result {
                    view.has_models = value["hasModels"].as_bool();
                    view.resolved_group = value["resolvedGroup"].as_str().map(str::to_owned);
                    let mut lines = vec![value["project"].as_str().unwrap_or("").to_string()];
                    lines.extend(value["rows"].as_array().into_iter().flatten().map(|row| {
                        format!(
                            "{}  {:.1}%",
                            row["name"].as_str().unwrap_or(""),
                            row["pct"].as_f64().unwrap_or(0.)
                        )
                    }));
                    view.mark_tooltip = lines.join("\n");
                }
                match result.and_then(|value| crate::live_mark::ProjectMark::from_response(&value))
                {
                    Ok(mark) => {
                        if let Some(id) = id {
                            if let Some(old) = view.project_marks.insert(id, mark.clone()) {
                                crate::image_assets::release(old.image, cx);
                            }
                            let protected = view.nearby_project_marks(cx);
                            view.trim_project_marks(&protected, cx);
                        }
                        view.live_mark.set_project(Some(mark.clone()), cx);
                        view.center_mark.set_project(Some(mark), cx);
                    }
                    Err(error) => view.notice = Some(error),
                }
                cx.notify();
            });
        }));
    }

    fn nearby_project_marks(&self, cx: &App) -> Vec<String> {
        let top = self.project_scroll.top_item().saturating_sub(12);
        let mut ids = self
            .picker_items(cx)
            .into_iter()
            .skip(top)
            .take(64)
            .filter(|id| self.projects.iter().any(|project| &project.id == id))
            .collect::<Vec<_>>();
        if let Some(id) = &self.selected
            && !ids.contains(id)
        {
            ids.push(id.clone());
        }
        ids
    }

    fn trim_project_marks(&mut self, protected: &[String], cx: &mut App) {
        while self.project_marks.len() > 128 {
            let id = self
                .project_marks
                .keys()
                .find(|id| self.selected.as_ref() != Some(id) && !protected.contains(id))
                .cloned();
            let Some(id) = id else { break };
            if let Some(mark) = self.project_marks.remove(&id) {
                crate::image_assets::release(mark.image, cx);
            }
            self.mark_cache_keys.remove(&id);
        }
        // Failed/unreadable rows have no image. Retain their attempted key only
        // for this neighborhood so scrolling does not create an idle retry loop.
        self.mark_cache_keys
            .retain(|id, _| self.project_marks.contains_key(id) || protected.contains(id));
    }

    pub(super) fn fill_project_marks(&mut self, cx: &mut Context<Self>) {
        if self.fixture
            || self.panel != Some(Panel::Projects)
            || self.row_mark_cancellation.is_some()
        {
            return;
        }
        let wanted = self.nearby_project_marks(cx);
        self.trim_project_marks(&wanted, cx);
        let mut keys = BTreeMap::new();
        let requests = self
            .projects
            .iter()
            .filter(|project| wanted.contains(&project.id))
            .filter_map(|project| {
                let group = self
                    .preferences
                    .projects
                    .get(&project.id)
                    .map(|prefs| prefs.group.clone())
                    .unwrap_or_else(|| "auto".into());
                let lang = self.preferences.language.code().to_string();
                let named = self.preferences.named;
                let key = format!("{:?}/{group}/{lang}/{named}", project.last_analyzed);
                if self.mark_cache_keys.get(&project.id) == Some(&key) {
                    return None;
                }
                keys.insert(project.id.clone(), key);
                Some(crate::project_marks::RowRequest {
                    id: project.id.clone(),
                    group,
                    named,
                    lang,
                })
            })
            .collect::<Vec<_>>();
        if requests.is_empty() {
            return;
        }
        self.row_mark_generation = self.row_mark_generation.wrapping_add(1);
        let generation = self.row_mark_generation;
        let cancellation = renderer::Cancellation::default();
        self.row_mark_cancellation = Some(cancellation.clone());
        let (tx, rx) = async_channel::bounded(4);
        self.row_mark_worker = Some(cx.background_spawn(async move {
            crate::project_marks::fill(requests, cancellation, |id, result| {
                tx.send_blocking((id, result)).is_ok()
            });
        }));
        self.row_mark_task = Some(cx.spawn(async move |this, cx| {
            while let Ok((id, result)) = rx.recv().await {
                let key = keys.get(&id).cloned();
                if this
                    .update(cx, |view, cx| {
                        if generation != view.row_mark_generation
                            || !view.projects.iter().any(|p| p.id == id)
                        {
                            return;
                        }
                        if let Some(key) = key {
                            view.mark_cache_keys.insert(id.clone(), key);
                        }
                        if let Ok(mark) = result
                            .and_then(|value| crate::live_mark::ProjectMark::from_response(&value))
                        {
                            if let Some(old) = view.project_marks.insert(id, mark) {
                                crate::image_assets::release(old.image, cx);
                            }
                            let protected = view.nearby_project_marks(cx);
                            view.trim_project_marks(&protected, cx);
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
            let _ = this.update(cx, |view, cx| {
                if generation == view.row_mark_generation {
                    view.row_mark_cancellation = None;
                    // A search/scroll during the batch may have changed the
                    // visible neighborhood; continue only for those new rows.
                    view.fill_project_marks(cx);
                }
            });
        }));
    }

    pub(super) fn fetch_details(&mut self, cx: &mut Context<Self>) {
        if self.fixture {
            return;
        }
        let Some(id) = self.settings_id().cloned() else {
            return;
        };
        let selection = id.clone();
        self.details_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { service::project_details(id) })
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.settings_id() != Some(&selection) {
                    return;
                }
                match result {
                    Ok(details) => {
                        if view.selected.as_ref() == Some(&selection) {
                            view.details = Some(details);
                        } else {
                            view.settings_details = Some(details);
                        }
                    }
                    Err(error) => {
                        view.error = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn load_blame(&mut self, cx: &mut Context<Self>) {
        if self.operation_busy {
            return;
        }
        let Some(id) = self.selected.clone().filter(|id| id != "__all__") else {
            return;
        };
        let file = self.blame_file.read(cx).value().to_string();
        let line = match self.blame_line.read(cx).value().parse::<u32>() {
            Ok(line) if line > 0 => line,
            _ => {
                self.error = Some(
                    self.text("请输入正整数行号", "Enter a positive line number")
                        .into(),
                );
                cx.notify();
                return;
            }
        };
        let selection = id.clone();
        self.operation_busy = true;
        self.operation_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { service::blame(id, file, u64::from(line)) })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.operation_busy = false;
                if view.selected.as_ref() != Some(&selection) {
                    return;
                }
                match result {
                    Ok(blame) => {
                        view.blame = Some(blame);
                        view.select_piece("blame".into(), cx);
                    }
                    Err(error) => {
                        view.error = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn export_evidence(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.settings_id().cloned() else {
            return;
        };
        let directory = match service::default_export_dir() {
            Ok(path) => path,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let prompt = cx.prompt_for_new_path(&directory, Some("creditia-evidence.json"));
        self.dialog_task = Some(cx.spawn(async move |this, cx| {
            let choice = prompt.await;
            let path = match choice {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    let _ = this.update(cx, |view, cx| {
                        view.error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
                Err(error) => {
                    let _ = this.update(cx, |view, cx| {
                        view.error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };
            let result = cx
                .background_spawn(async move {
                    let evidence = service::evidence_archive(id).map_err(|e| e.to_string())?;
                    let bytes = serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?;
                    renderer::write_export(&path, &bytes, renderer::Cancellation::default())
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                match result {
                    Ok(()) => view.show_success(
                        view.text("证据档案已保存", "Evidence archive saved").into(),
                        cx,
                    ),
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn import_evidence(&mut self, cx: &mut Context<Self>) {
        if self.job_busy || self.automatic_job.is_some() {
            return;
        }
        let Some(id) = self.settings_id().cloned() else {
            return;
        };
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(self.text("导入证据档案", "Import evidence archive").into()),
        });
        self.dialog_task = Some(cx.spawn(async move |this, cx| {
            let choice = prompt.await;
            let path = match choice {
                Ok(Ok(Some(paths))) => match paths.into_iter().next() {
                    Some(path) => path,
                    None => return,
                },
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    let _ = this.update(cx, |view, cx| {
                        view.error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
                Err(error) => {
                    let _ = this.update(cx, |view, cx| {
                        view.error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };
            let _ = this.update(cx, |view, cx| view.start_evidence_import(id, path, cx));
        }));
    }

    fn start_evidence_import(&mut self, id: String, path: PathBuf, cx: &mut Context<Self>) {
        if self.job_busy || self.automatic_job.is_some() {
            self.notice = Some(
                self.text(
                    "已有任务正在运行，请稍后重试",
                    "Another task is running; try again when it finishes",
                )
                .into(),
            );
            cx.notify();
            return;
        }
        let cancellation = service::AnalysisCancellation::default();
        self.analysis_cancellation = Some(cancellation.clone());
        self.job_busy = true;
        self.job_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { service::import_evidence(id, path, cancellation) })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.job_busy = false;
                view.cancelling = false;
                view.analysis_cancellation = None;
                match result {
                    Ok(_) => {
                        view.notice = Some(
                            view.text(
                                "证据已导入，重新分析后更新作品",
                                "Evidence imported; analyze to update artwork",
                            )
                            .into(),
                        );
                        view.fetch_details(cx);
                    }
                    Err(error) => view.error = Some(error.to_string()),
                }
                view.start_next_project(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn request_diagnostics(&mut self, cx: &mut Context<Self>) {
        self.report = None;
        if self.fixture {
            return;
        }
        self.report_task = Some(cx.spawn(async move |this, cx| {
            let report = cx.background_spawn(async { service::doctor() }).await;
            let _ = this.update(cx, |view, cx| {
                view.report = Some(report);
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn operation(
        &mut self,
        work: impl FnOnce() -> Result<Value, service::Fail> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.fixture || self.operation_busy {
            return;
        }
        self.operation_busy = true;
        self.error = None;
        self.operation_task = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { work() }).await;
            let _ = this.update(cx, |view, cx| {
                view.operation_busy = false;
                view.collect = service::collect_info();
                match result {
                    Ok(_) => {
                        view.show_success(view.text("已更新", "Updated").into(), cx);
                        view.fetch_details(cx);
                    }
                    Err(error) => {
                        view.error = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn save_project_display(&mut self, cx: &mut Context<Self>) {
        if self.operation_busy || self.fixture {
            return;
        }
        let Some(id) = self.settings_id().cloned() else {
            return;
        };
        let selection = id.clone();
        let sign = self.sign_input.read(cx).value().to_string();
        let hidden = !self.preferences.named;
        self.operation_busy = true;
        self.operation_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    service::set_display(id.clone(), hidden, sign)?;
                    service::project_details(id)
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.operation_busy = false;
                match result {
                    Ok(details) => {
                        if view.selected.as_ref() == Some(&selection) {
                            view.details = Some(details);
                            view.rebuild_preserving_view(cx);
                        } else if view.settings_id() == Some(&selection) {
                            view.settings_details = Some(details);
                        }
                        view.show_success(
                            view.text("显示设置已保存", "Display settings saved").into(),
                            cx,
                        );
                    }
                    Err(error) => {
                        view.error = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn set_integration(&mut self, which: &str, on: bool, cx: &mut Context<Self>) {
        let Some(id) = self.settings_id().cloned() else {
            return;
        };
        let which = which.to_string();
        let lang = self.preferences.language.code().to_string();
        self.operation(
            move || service::set_integration(id, which, on, Some(lang)),
            cx,
        );
    }

    pub(super) fn remove_project(&mut self, cx: &mut Context<Self>) {
        if self.fixture {
            return;
        }
        if !self.remove_confirm {
            self.remove_confirm = true;
            cx.notify();
            return;
        }
        let Some(id) = self.settings_id().cloned() else {
            return;
        };
        self.panel = None;
        let removed = id.clone();
        self.operation_busy = true;
        self.operation_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { service::remove_project(id) })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.operation_busy = false;
                match result {
                    Ok(()) => {
                        if view.selected.as_ref() == Some(&removed) {
                            view.selected = None;
                            view.raw = None;
                        }
                        view.refresh(cx);
                    }
                    Err(error) => {
                        view.error = Some(crate::ui_reports::failure(
                            &error,
                            &view.preferences.language,
                        ))
                    }
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn link_folder(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.settings_id().cloned() else {
            return;
        };
        let lang = self.preferences.language.code().to_string();
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(self.text("关联项目路径", "Link project path").into()),
        });
        self.dialog_task = Some(cx.spawn(async move |this, cx| {
            let choice = prompt.await;
            let _ = this.update(cx, |view, cx| {
                match choice {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            let path = path.to_string_lossy().into_owned();
                            view.operation(
                                move || service::link_path(id, path, Some(false), Some(lang)),
                                cx,
                            );
                        }
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => view.error = Some(error.to_string()),
                    Err(error) => view.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn unlink_path(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(id) = self.settings_id().cloned() else {
            return;
        };
        self.operation(move || service::link_path(id, path, Some(true), None), cx);
    }

    pub(super) fn show_all_projects(&mut self, cx: &mut Context<Self>) {
        if self.fixture {
            return;
        }
        if self.selected.as_deref() != Some("__all__") {
            self.focus_styles = None;
        }
        self.pending_view = if self.selected.as_deref() == Some("__all__") && self.raw.is_some() {
            Some(self.bookmark())
        } else {
            None
        };
        self.clear_preview(cx);
        self.archive_generation = self.archive_generation.wrapping_add(1);
        let generation = self.archive_generation;
        self.selected = Some("__all__".into());
        self.preferences.last_project = self.selected.clone();
        self.persist();
        if let Some(background) = &self.background {
            background.watch_project(Some("__all__".into()));
        }
        self.details = None;
        self.loading = true;
        self.panel = None;
        self.raw = None;
        self.render_archive = None;
        let lang = self.preferences.language.code().to_string();
        self.archive_task = Some(cx.spawn(async move |this, cx| {
            let result: Result<(Value, Vec<Value>), String> = cx
                .background_spawn(async move {
                    let collection = service::project_archives().map_err(|e| e.to_string())?;
                    let projects = collection
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|p| p.get("raw").filter(|v| v.is_object()).cloned())
                        .collect::<Vec<_>>();
                    if projects.is_empty() {
                        return Err("No project archives are available".into());
                    }
                    let result = renderer::request(
                        json!({
                            "version": 1,
                            "id": "all-projects",
                            "op": "merge",
                            "projects": projects,
                            "lang": lang
                        }),
                        renderer::Cancellation::default(),
                    )?;
                    Ok((result["raw"].clone(), projects))
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.archive_generation != generation {
                    return;
                }
                view.loading = false;
                match result {
                    Ok((raw, projects)) => {
                        view.raw = Some(Arc::new(raw));
                        view.all_archives = Arc::new(projects);
                        view.load_catalog(cx);
                    }
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn track_project_marks(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.panel == Some(Panel::Projects) {
            for id in self.nearby_project_marks(cx) {
                if let Some(mark) = self.project_marks.get(&id) {
                    crate::image_assets::track(&mark.image, window, cx);
                }
            }
        }
    }
}
