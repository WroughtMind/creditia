//! Real production input/preview integration: no pre-rendered fixture artwork.
//! File persistence, analysis services and animation playback have separate gates.
use creditia_gpui::app::Creditia;
use creditia_gpui::{Appearance, Language, Preferences};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, ElementId, Entity, HeadlessAppContext, InputEvent,
    MouseMoveEvent, PinchEvent, ScrollDelta, ScrollEvent, TouchPhase, Window, WindowBounds,
    WindowOptions, assets::Assets, point, px, size,
};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

struct Harness {
    cx: HeadlessAppContext,
    handle: AnyWindowHandle,
    view: Entity<Creditia>,
    catalog: Value,
    output: PathBuf,
    key: String,
    artifacts: Vec<Value>,
    checks: Vec<Value>,
    feature_checks: Vec<Value>,
}
impl Harness {
    fn new(
        raw: Value,
        catalog: Value,
        output: &Path,
        key: &str,
        viewport: (f32, f32),
        preferences: Preferences,
    ) -> Self {
        let mut cx = HeadlessAppContext::with_platform(
            gpui_kit::platform::current_platform(true).text_system(),
            Arc::new(Assets),
            gpui_kit::platform::current_headless_renderer,
        );
        cx.update(gpui_kit::init);
        cx.update(|cx| creditia_gpui::app::init_fonts(cx).expect("bundled UI fonts"));
        let (handle, view) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds {
                            origin: Default::default(),
                            size: size(px(viewport.0), px(viewport.1)),
                        })),
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    cx,
                    |window, cx| cx.new(|cx| Creditia::benchmark(raw, preferences, window, cx)),
                )
            })
            .expect("production benchmark window opens");
        Self {
            cx,
            handle,
            view,
            catalog,
            output: output.to_path_buf(),
            key: key.into(),
            artifacts: vec![],
            checks: vec![],
            feature_checks: vec![],
        }
    }
    fn update<R>(&mut self, f: impl FnOnce(&mut Window, &mut App, &Entity<Creditia>) -> R) -> R {
        let view = self.view.clone();
        self.cx
            .update_window(self.handle, |_, window, cx| f(window, cx, &view))
            .unwrap()
    }
    fn state(&mut self) -> Value {
        self.update(|_, cx, view| view.read(cx).diagnostic_state())
    }
    fn click(&mut self, id: impl Into<ElementId>) {
        let id = id.into();
        self.update(|window, cx, _| window.click(id, cx));
    }
    fn press(&mut self, key: &str) {
        self.update(|window, cx, _| window.press(key, cx));
    }
    fn shortcut(&mut self, key: &str) {
        self.press(&format!(
            "{}-{key}",
            if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            }
        ));
    }
    fn export_format(&mut self, id: &str) {
        if self.state()["export_formats_open"] != true {
            self.click("export-format");
        }
        self.click(format!("export-format-{id}"));
    }
    fn ready(&mut self) {
        let started = Instant::now();
        loop {
            self.cx.run_until_parked();
            let (ready, state) = self.update(|window, cx, view| {
                // The test window has no display link to request the final
                // settled-resolution/crop frame after geometry changes.
                view.update(cx, |_, cx| cx.notify());
                window.render_frame(cx);
                let ready = view.update(cx, |view, cx| view.fixture_images_ready(window, cx));
                (ready, view.read(cx).diagnostic_state())
            });
            assert!(
                state["error"].is_null(),
                "production renderer reported an error: {state}"
            );
            if ready && state["tile_count"].as_u64().unwrap_or(0) > 0 {
                if state["focused"] == true {
                    assert!(
                        state["art_width"].as_f64().is_some_and(|width| width > 0.)
                            && state["art_height"]
                                .as_f64()
                                .is_some_and(|height| height > 0.),
                        "focused runtime fixture unexpectedly produced empty artwork: {state}"
                    );
                }
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(90),
                "real shared-renderer assets did not decode: {state}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn settle(&mut self) {
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(220) {
            self.cx.run_until_parked();
            self.update(|window, cx, view| {
                // Headless windows have no display-link callbacks. Invalidate
                // once per sampled frame so real-time transitions can finish.
                view.update(cx, |_, cx| cx.notify());
                window.render_frame(cx);
            });
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn setup(&mut self) {
        self.ready();
        assert_eq!(self.state()["view"], "wall");
        assert_eq!(self.state()["global_style"], "minimal-light");
        self.click("settings");
        self.settle();
        assert_eq!(self.state()["panel"], "settings");
        if self.state()["reduced_motion"] != true {
            self.click("reduced-motion");
        }
        assert_eq!(self.state()["reduced_motion"], true);
        self.click("panel-close");
        assert!(self.state()["panel"].is_null());
        self.ready();
        let state = self.state();
        self.check(
            "actual-renderer-startup-and-settings-motion",
            json!({"state":state,"supplied_artworks":false}),
        );
    }
    fn check(&mut self, id: &str, detail: Value) {
        self.checks
            .push(json!({"id":id,"passed":true,"detail":detail}));
    }
    fn feature(&mut self, id: &str, check: &str, behavior: &str) {
        self.feature_checks.push(json!({"feature_id":id,"check":check,"passed":true,"behavior":behavior,"test_id":self.key}));
    }
    fn style(&self, id: &str) -> &Value {
        self.catalog["styles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|style| style["id"] == id)
            .unwrap()
    }
    fn choose_style(&mut self, id: &str) {
        let index = self.catalog["styles"]
            .as_array()
            .unwrap()
            .iter()
            .position(|style| style["id"] == id)
            .unwrap();
        self.click("styles");
        assert_eq!(self.state()["panel"], "styles");
        self.click(("style", index));
        assert_eq!(self.state()["style"], id);
        assert!(self.state()["panel"].is_null());
        self.ready();
    }
    fn focus(&mut self, piece: &str) {
        if self.state()["export_workspace"] == true {
            self.click("export-back");
            self.ready();
        }
        if self.state()["focused"] == true {
            self.press("escape");
            self.ready();
        }
        let style = self.state()["style"].as_str().unwrap().to_string();
        let index = self.style(&style)["pieces"]
            .as_array()
            .unwrap()
            .iter()
            .position(|item| item["id"] == piece)
            .unwrap();
        self.click("pieces");
        assert_eq!(self.state()["panel"], "pieces");
        self.click(("piece", index));
        assert_eq!(self.state()["selected_piece"], piece);
        assert_eq!(self.state()["focused"], true);
        assert!(self.state()["panel"].is_null());
        self.ready();
    }
    fn capture(&mut self, label: &str, artwork_only: bool) -> Vec<u8> {
        let bounds = self.update(|window, cx, view| {
            // Move the pointer away from artwork so tooltip/caption changes
            // cannot masquerade as a regenerated artwork in pixel comparisons.
            window.hover(
                if view.read(cx).diagnostic_state()["export_workspace"] == true {
                    "export-format"
                } else {
                    "styles"
                },
                cx,
            );
            window.render_frame(cx);
            if artwork_only {
                let state = view.read(cx).diagnostic_state();
                assert_eq!(state["focused"], true);
                let picture =
                    window.find(("artwork", state["selected_tile"].as_u64().unwrap() as usize));
                assert!(picture.visible());
                Some(picture.bounds())
            } else {
                None
            }
        });
        let image = self
            .cx
            .capture_screenshot(self.handle)
            .expect("real native screenshot");
        let name = format!("{}-{label}.png", self.key);
        image.save(self.output.join(&name)).unwrap();
        self.artifacts.push(json!({"path":name}));
        let scale = image.width() as f32 / 1440.;
        let (x, y, w, h) = if let Some(bounds) = bounds {
            let left: f32 = bounds.origin.x.into();
            let top: f32 = bounds.origin.y.into();
            let width: f32 = bounds.size.width.into();
            let height: f32 = bounds.size.height.into();
            assert!(
                left >= 0. && top >= 0. && left + width <= 1441. && top + height <= 1001.,
                "test artwork must fit within the captured window"
            );
            let x = (left * scale).round() as u32;
            let y = (top * scale).round() as u32;
            (
                x,
                y,
                ((width * scale).round() as u32).min(image.width() - x),
                ((height * scale).round() as u32).min(image.height() - y),
            )
        } else {
            (0, 0, image.width(), image.height())
        };
        let mut pixels = Vec::with_capacity((w * h * 4) as usize + 8);
        pixels.extend_from_slice(&w.to_le_bytes());
        pixels.extend_from_slice(&h.to_le_bytes());
        let mut colors = HashSet::new();
        for yy in y..y + h {
            for xx in x..x + w {
                let pixel = image.get_pixel(xx, yy).0;
                pixels.extend_from_slice(&pixel);
                colors.insert(pixel);
            }
        }
        assert!(
            colors.len() > 8,
            "decoded preview has no visible artwork detail"
        );
        pixels
    }
    fn artwork_pixels(&mut self) -> (gpui_kit::Size<gpui_kit::DevicePixels>, Vec<u8>) {
        self.update(|window, cx, view| {
            let image = view
                .update(cx, |view, cx| view.fixture_artwork_image(window, cx))
                .expect("decoded artwork is ready");
            (
                image.size(0),
                image.as_bytes(0).expect("artwork has pixels").to_vec(),
            )
        })
    }
    fn failure(&mut self, error: String) -> Value {
        let state = self
            .cx
            .update_window(self.handle, |_, _, cx| {
                self.view.read(cx).diagnostic_state()
            })
            .unwrap_or_else(|error| json!({"window_unavailable":error.to_string()}));
        if let Ok(image) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.cx
                .capture_screenshot(self.handle)
                .expect("failure screenshot")
        })) {
            let name = format!("{}-failed.png", self.key);
            if image.save(self.output.join(&name)).is_ok() {
                self.artifacts.push(json!({"path":name}));
            }
        }
        json!({"key":self.key,"status":"failed","error":error,"state":state,"checks":self.checks,"feature_checks":[],"artifacts":self.artifacts})
    }
}

fn layouts(h: &mut Harness) {
    h.focus("tier");
    let variants: Vec<String> = h.style("minimal-light")["pieces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|piece| piece["id"] == "tier")
        .unwrap()["variants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|variant| variant["id"].as_str().unwrap().to_string())
        .collect();
    assert!(variants.len() > 2);
    h.click("focus-layout");
    assert_eq!(h.state()["panel"], "layouts");
    h.click(("variant", 0usize));
    h.ready();
    assert_eq!(h.state()["selected_variant"], variants[0]);
    let first = h.capture("first-layout", true);
    h.press("left");
    h.ready();
    assert_eq!(
        h.state()["selected_variant"],
        *variants.last().unwrap(),
        "left after modal closure must wrap to the last layout"
    );
    let last = h.capture("wrapped-layout", true);
    assert_ne!(
        first, last,
        "different layouts must paint different artwork pixels"
    );
    h.press("right");
    h.ready();
    assert_eq!(h.state()["selected_variant"], variants[0]);
    h.press("right");
    h.ready();
    assert_eq!(h.state()["selected_variant"], variants[1]);
    h.press("right");
    h.ready();
    assert_eq!(h.state()["selected_variant"], variants[2]);
    h.capture("settled-third-layout", true);
    let settled = h.artwork_pixels();
    let settled_rect = h.state()["focused_rect"].clone();
    h.click("focus-layout");
    h.click(("variant", variants.len() - 1));
    // Intentionally issue new selections before the background renderer settles.
    h.press("right");
    h.press("right");
    h.press("right");
    h.ready();
    assert_eq!(h.state()["selected_variant"], variants[2]);
    h.capture("rapid-final-layout", true);
    let rapid = h.artwork_pixels();
    assert_eq!(h.state()["focused_rect"], settled_rect);
    // swap_variant can rearrange other wall cards behind the transparent
    // corners. Compare the actual foreground image bound to GPUI, not that
    // path-dependent composited background. No pixel tolerance is applied.
    assert!(
        settled == rapid,
        "obsolete render responses must not replace the latest layout pixels"
    );
    h.press("escape");
    assert_eq!(h.state()["focused"], false);
    let state = h.state();
    h.check(
        "layout-selector-keyboard-focus-and-bidirectional-wrap",
        json!({"layouts":variants,"state":state,"artwork_pixels_changed":true}),
    );
    h.feature("variant", "前后版式循环", "真实版式选择后 Left 从首项循环到末项，Right 从末项返回首项并继续下一项，解码后的作品像素发生变化");
    h.feature(
        "variant",
        "连续点击只应用最后一次",
        "不等待后台绘制即连续切换三次版式；最终选择及原生作品像素与独立稳定绘制的目标版式完全一致",
    );
    h.feature(
        "keyboard",
        "对话框关闭后焦点返回",
        "版式模态选择关闭后，Left/Right 直接控制当前作品并保持正确焦点",
    );
    h.feature(
        "keyboard",
        "局部方向键与 Escape",
        "当前作品响应 Left/Right 版式导航，Escape 返回展墙",
    );
}
fn trial_restore(h: &mut Harness) {
    h.focus("tier");
    let before = h.capture("original", true);
    h.choose_style("ink");
    assert_eq!(h.state()["global_style"], "minimal-light");
    assert_eq!(h.state()["preview_style"], true);
    assert_eq!(h.state()["selected_piece"], "tier");
    let trial = h.capture("ink-trial", true);
    assert_ne!(before, trial);
    h.press("escape");
    h.ready();
    assert_eq!(h.state()["focused"], false);
    assert_eq!(h.state()["style"], "minimal-light");
    assert_eq!(h.state()["global_style"], "minimal-light");
    assert_eq!(h.state()["preview_style"], false);
    h.capture("restored-wall", false);
    let selected = h.state()["selected_tile"].as_u64().unwrap();
    h.press("down");
    assert_eq!(h.state()["selected_tile"].as_u64().unwrap(), selected + 1);
    h.press("up");
    assert_eq!(h.state()["selected_tile"].as_u64().unwrap(), selected);
    h.focus("tier");
    let restored = h.capture("restored-artwork", true);
    assert_eq!(
        before, restored,
        "canceling a style trial must restore original artwork pixels"
    );
    h.check(
        "focused-style-trial-and-escape-restoration",
        json!({"trial_style":"ink","global_style":"minimal-light","restored_pixels_equal":true}),
    );
    h.feature(
        "focus",
        "局部试用其它支持该作品的风格",
        "放大 tier 后试用 ink；实际作品像素改变而全局风格不变，Escape 恢复原风格与原作品像素",
    );
    h.feature(
        "focus",
        "关闭后恢复原键盘焦点",
        "Escape 关闭临时风格放大视图后，Down/Up 立即控制展墙中的作品选择",
    );
}
fn apply_style(h: &mut Harness) {
    h.focus("tier");
    h.choose_style("ink");
    h.click("apply-focus-style");
    h.ready();
    assert_eq!(h.state()["focused"], false);
    assert_eq!(h.state()["global_style"], "ink");
    assert_eq!(h.state()["style"], "ink");
    assert_eq!(h.state()["preview_style"], false);
    h.capture("applied-wall", false);
    h.focus("tier");
    assert_eq!(h.state()["style"], "ink");
    h.capture("applied-tier", true);
    h.press("escape");
    h.ready();
    assert_eq!(h.state()["global_style"], "ink");
    let state = h.state();
    h.check("apply-focused-style-to-global-view", json!({"state":state}));
    h.feature(
        "focus",
        "设为全局风格",
        "放大作品试用 ink 后点击设为全局；返回展墙、重新放大及再次关闭均保持 ink",
    );
}
fn groups(h: &mut Harness) {
    h.click("group-tool");
    h.ready();
    assert_eq!(h.state()["group"], "tool");
    h.focus("tier");
    let tools = h.capture("tool-groups", true);
    h.press("escape");
    h.click("group-model");
    h.ready();
    assert_eq!(h.state()["group"], "model");
    h.focus("tier");
    let models = h.capture("model-groups", true);
    assert_ne!(
        tools, models,
        "grouping must update artwork pixels, not only toolbar selection"
    );
    let state = h.state();
    h.check(
        "grouping-rebuilds-real-artwork",
        json!({"tool_and_model_pixels_differ":true,"state":state}),
    );
}
fn preferences(h: &mut Harness) {
    h.focus("people");
    let private = h.capture("private-names", true);
    h.shortcut(",");
    assert_eq!(h.state()["panel"], "settings");
    h.click("settings-tab-general");
    h.click("names-settings");
    assert_eq!(h.state()["named"], true);
    h.click("panel-close");
    h.ready();
    let named = h.capture("named-people", true);
    assert_ne!(
        private, named,
        "name visibility must regenerate artwork pixels"
    );
    h.shortcut(",");
    h.click("settings-tab-general");
    h.click("settings-language-zh");
    assert_eq!(h.state()["language"], "zh");
    h.click("panel-close");
    h.ready();
    let chinese = h.capture("chinese-people", true);
    assert_ne!(named, chinese, "language must regenerate artwork pixels");
    let light = h.capture("light-theme", false);
    h.shortcut(",");
    h.click("appearance");
    assert_eq!(h.state()["appearance"], "dark");
    h.click("panel-close");
    h.ready();
    let dark = h.capture("dark-theme", false);
    assert_ne!(light, dark, "theme input must change actual native pixels");
    let state = h.state();
    h.check("names-language-and-theme-real-rendering",json!({"state":state,"names_artwork_changed":true,"language_artwork_changed":true,"native_theme_changed":true}));
    h.feature(
        "settings-general",
        "中英文",
        "设置中的语言按钮把英文切换为中文，状态与重新渲染的贡献者作品像素同步改变",
    );
    h.feature(
        "settings-export",
        "是否显示其他贡献者姓名",
        "通过真实设置切换姓名可见性，贡献者作品经后台渲染后像素发生变化",
    );
}
fn hover_first_hit(h: &mut Harness, first: &Value) {
    h.update(|window, cx, view| {
        let state = view.read(cx).diagnostic_state();
        let picture = window.find(("artwork", state["selected_tile"].as_u64().unwrap() as usize));
        let bounds = picture.bounds();
        let left: f32 = bounds.origin.x.into();
        let top: f32 = bounds.origin.y.into();
        let width: f32 = bounds.size.width.into();
        let height: f32 = bounds.size.height.into();
        let x = left
            + (first["x"].as_f64().unwrap() + first["w"].as_f64().unwrap() / 2.) as f32
                / state["art_width"].as_f64().unwrap() as f32
                * width;
        let y = top
            + (first["y"].as_f64().unwrap() + first["h"].as_f64().unwrap() / 2.) as f32
                / state["art_height"].as_f64().unwrap() as f32
                * height;
        window.dispatch_event(
            MouseMoveEvent {
                position: point(px(x), px(y)),
                pressed_button: None,
                modifiers: window.modifiers(),
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    });
    assert_eq!(
        h.state()["hover"],
        first["t"],
        "pointer hit testing must use actual artwork coordinates"
    );
}
fn spatial(h: &mut Harness) {
    h.focus("tier");
    let first = h.state()["first_hit"].clone();
    assert!(
        first.is_object(),
        "tier fixture needs an actual hover hit region"
    );
    hover_first_hit(h, &first);
    h.shortcut("=");
    h.ready();
    h.update(|window, cx, _| window.drag(point(px(700.), px(450.)), point(px(765.), px(485.)), cx));
    h.ready();
    hover_first_hit(h, &first);
    let name = format!("{}-actual-hover.png", h.key);
    h.cx.capture_screenshot(h.handle)
        .unwrap()
        .save(h.output.join(&name))
        .unwrap();
    h.artifacts.push(json!({"path":name}));
    h.press("escape");
    h.click("roam");
    h.ready();
    let before = h.state();
    h.update(|window, cx, _| window.drag(point(px(700.), px(450.)), point(px(860.), px(560.)), cx));
    h.ready();
    let after = h.state();
    assert!((after["pan_x"].as_f64().unwrap() - before["pan_x"].as_f64().unwrap()).abs() > 100.);
    assert!((after["pan_y"].as_f64().unwrap() - before["pan_y"].as_f64().unwrap()).abs() > 50.);
    let zoom = after["zoom"].as_f64().unwrap();
    h.shortcut("=");
    h.ready();
    assert!(h.state()["zoom"].as_f64().unwrap() > zoom);
    h.capture("roam-drag-zoom", false);
    h.click("zoom-reset");
    h.ready();
    let home = h.state();
    h.press("right");
    assert_ne!(h.state()["pan_x"], home["pan_x"]);
    h.press("0");
    assert_eq!(h.state()["pan_x"], home["pan_x"]);
    assert_eq!(h.state()["pan_y"], home["pan_y"]);
    h.press("down");
    assert_ne!(h.state()["pan_y"], home["pan_y"]);
    h.press("home");
    assert_eq!(h.state()["pan_x"], home["pan_x"]);
    assert_eq!(h.state()["pan_y"], home["pan_y"]);
    h.ready();
    h.shortcut("e");
    assert_eq!(h.state()["export_open"], true);
    h.update(|window, _, _| assert!(window.find("export-workspace").visible()));
    h.capture("export-workspace", false);
    h.press("escape");
    assert_eq!(h.state()["export_open"], false);
    let state = h.state();
    h.check("actual-hover-roam-drag-zoom-and-export-workspace",json!({"hover_title":first["t"],"drag_before":before,"drag_after":after,"state":state,"export_file_written":false}));
    h.focus("share");
    for _ in 0..10 {
        h.shortcut("=");
    }
    h.ready();
    let state = h.state();
    let region = &state["focused_preview"]["region"];
    assert!(
        region.is_object(),
        "deep zoom must exercise the actual cropped preview path"
    );
    let rect = &state["focused_rect"];
    let scale = rect["w"].as_f64().unwrap() / state["art_width"].as_f64().unwrap();
    assert!(
        rect["y"].as_f64().unwrap() + region["y"].as_f64().unwrap() * scale <= -59.5,
        "static preview must cover the strip behind the toolbar"
    );
    h.capture("deep-zoom-top-coverage", false);
    h.feature(
        "hover",
        "平移和缩放后命中准确",
        "通过真实指针在原尺寸及放大并拖动后的作品命中同一逻辑区域，实际浮层标题与该命中区域一致",
    );
    h.feature(
        "roam",
        "方向键移动、Home/0 回原点",
        "漫游中 Right/Down 改变对应坐标，0 与 Home 均恢复已记录的原点坐标",
    );
}

fn reviewed_interactions(h: &mut Harness) {
    h.click("roam");
    h.ready();
    let before = h.state();
    let center = (970., 360.);
    h.update(|window, cx, _| {
        // No preceding pointer move: the gesture's actual position is the anchor.
        window.dispatch_event(
            PinchEvent {
                position: point(px(center.0), px(center.1 + 60.)),
                delta: 0.4,
                phase: TouchPhase::Moved,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    });
    let zoomed = h.state();
    let z0 = before["zoom"].as_f64().unwrap();
    let z1 = zoomed["zoom"].as_f64().unwrap();
    assert!((z1 / z0 - 1.4).abs() < 0.001);
    for (field, anchor) in [("pan_x", center.0), ("pan_y", center.1)] {
        let original = (anchor as f64 - before[field].as_f64().unwrap()) / z0;
        let current = (anchor as f64 - zoomed[field].as_f64().unwrap()) / z1;
        assert!(
            (original - current).abs() < 0.01,
            "pinch anchor moved: {field}"
        );
    }
    for _ in 0..6 {
        h.update(|window, cx, _| {
            window.dispatch_event(
                PinchEvent {
                    position: point(px(center.0), px(center.1 + 60.)),
                    delta: -0.08,
                    phase: TouchPhase::Moved,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
        });
    }
    let zoomed = h.state();
    let zoom = zoomed["zoom"].as_f64().unwrap();
    assert!(zoom < z0);
    for (field, anchor) in [("pan_x", center.0), ("pan_y", center.1)] {
        let original = (anchor as f64 - before[field].as_f64().unwrap()) / z0;
        let current = (anchor as f64 - zoomed[field].as_f64().unwrap()) / zoom;
        assert!(
            (original - current).abs() < 0.01,
            "continuous zoom-out anchor moved: {field}"
        );
    }
    h.update(|window, cx, _| {
        window.dispatch_event(
            ScrollEvent {
                position: point(px(970.), px(420.)),
                delta: ScrollDelta::Pixels(point(px(46.), px(-32.))),
                touch_phase: TouchPhase::Moved,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    });
    let panned = h.state();
    assert!(
        (panned["pan_x"].as_f64().unwrap() - zoomed["pan_x"].as_f64().unwrap() - 46.).abs() < 0.01
    );
    assert!(
        (panned["pan_y"].as_f64().unwrap() - zoomed["pan_y"].as_f64().unwrap() + 32.).abs() < 0.01
    );
    h.focus("tier");
    h.press("escape");
    let restored = h.state();
    for field in ["pan_x", "pan_y", "zoom"] {
        assert_eq!(restored[field], panned[field]);
    }
    h.check(
        "pointer-anchored-pinch-pan-and-focus-return",
        json!({"before":before,"zoomed":zoomed,"panned":panned,"restored":restored}),
    );

    h.focus("tier");
    let dock_width =
        h.update(|window, _, _| f32::from(window.find("focus-dock").bounds().size.width));
    // Start with the saved GIF preference. OS save prompts are deliberately
    // outside this input scenario; the test platform waits for their response.
    assert_eq!(h.state()["export_format"], "gif");
    h.focus("badge");
    h.capture("static-dock", false);
    h.update(|window, _, _| {
        assert!(window.find("static-artwork").visible());
        assert!(
            (f32::from(window.find("focus-dock").bounds().size.width) - dock_width).abs() < 0.01
        );
    });
    assert_eq!(h.state()["can_export"], false);
    h.click("focus-export");
    assert_eq!(
        h.state()["export_open"],
        true,
        "saved GIF cannot lock the export entry"
    );
    h.export_format("gif");
    assert_eq!(
        h.state()["export_open"],
        true,
        "unsupported GIF must not start an export"
    );
    assert_eq!(h.state()["export_format"], "png");
    h.ready();
    h.capture("static-export-with-gif-disabled", false);
    h.press("escape");
    assert_eq!(h.state()["export_formats_open"], false);
    assert_eq!(h.state()["export_open"], true);
    h.press("escape");
    assert_eq!(h.state()["export_open"], false);
    h.press("escape");
    h.click("wall");
    h.ready();
    h.focus("badge");
    h.press("escape");
    let rect = h.state()["selected_rect"].clone();
    let scroll = 450. - rect["y"].as_f64().unwrap() - rect["h"].as_f64().unwrap() / 2.;
    h.update(|window, cx, _| {
        window.dispatch_event(
            ScrollEvent {
                position: point(px(700.), px(450.)),
                delta: ScrollDelta::Pixels(point(px(0.), px(scroll as f32))),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    });
    h.ready();
    let index = h.state()["selected_tile"].as_u64().unwrap() as usize;
    h.update(|window, cx, _| window.hover(("artwork", index), cx));
    h.click(("tile-export", index));
    assert_eq!(
        h.state()["export_open"],
        true,
        "wall and focus must use the same availability rule"
    );
    h.press("escape");
    let state = h.state();
    h.check(
        "static-label-stable-dock-and-export-entry-with-saved-gif",
        json!({"state":state,"file_written":false,"initial_export_preference":"gif","system_save_dialog_exercised":false}),
    );

    // The continuous producer blocks while waiting for UI playhead updates.
    // A deterministic headless executor cannot run both on the same thread.
    // Completion and producer controls have separate unit checks; real-window
    // play/pause/resume/replay remains a manual acceptance step.
}

fn export_workspace(h: &mut Harness) {
    h.focus("tier");
    let before = h.state();
    h.click("focus-export");
    h.ready();
    assert_eq!(h.state()["export_workspace"], true);
    assert_eq!(h.state()["export_preview"]["image_ready"], true);
    assert_eq!(h.state()["focused_rect"], before["focused_rect"]);
    h.update(|window, _, _| {
        assert!(
            window
                .find((
                    "artwork",
                    before["selected_tile"].as_u64().unwrap() as usize
                ))
                .visible()
        );
        assert!(window.find("export-dock").visible());
    });
    h.capture("png", false);
    h.click("export-options");
    assert_eq!(h.state()["export_options_open"], true);
    h.click("export-format");
    assert_eq!(h.state()["export_options_open"], false);
    assert_eq!(h.state()["export_formats_open"], true);
    for id in ["svg", "json", "png"] {
        h.export_format(id);
    }
    h.ready();
    assert_eq!(h.state()["export_format"], "png");
    assert_eq!(h.state()["export_preview"]["image_ready"], true);
    assert_eq!(h.state()["export_preview"]["text_ready"], false);
    for id in ["svg", "pdf", "gif", "html"] {
        h.export_format(id);
        h.ready();
        let state = h.state();
        assert_eq!(state["export_format"], id);
        assert_eq!(
            state["export_prompt_busy"], false,
            "choosing a format only previews it"
        );
        assert!(
            state["export_preview"]["error"].is_null(),
            "preview failed: {state}"
        );
    }
    for id in ["embed", "md", "txt", "json"] {
        h.export_format(id);
        h.ready();
        let state = h.state();
        assert_eq!(
            state["export_preview"]["text_ready"], true,
            "text preview must come from the renderer: {state}"
        );
        h.click("export-copy");
        let copied = h.update(|_, cx, _| {
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .unwrap()
        });
        match id {
            "md" => assert!(
                copied.contains("<!-- creditia:start -->")
                    && copied.contains("<!-- creditia:end -->")
            ),
            "json" => assert_eq!(
                serde_json::from_str::<Value>(&copied).unwrap()["project"]["name"],
                "catnap"
            ),
            "embed" => assert!(copied.contains("credits.json") && copied.contains("creditia")),
            _ => assert!(copied.contains("catnap")),
        }
        assert_eq!(
            h.state()["export_workspace"],
            true,
            "copy remains in the current workspace"
        );
        h.capture(&format!("{id}-copied"), false);
    }
    assert_eq!(h.state()["success_notice"], "Copied");
    // The headless executor owns a virtual timer clock; wall-clock sleeping
    // cannot expire production executor timers. Re-copy also replaces its timer.
    h.cx.run_until_parked();
    h.cx.advance_clock(Duration::from_secs(2));
    h.click("export-copy");
    h.cx.run_until_parked();
    h.cx.advance_clock(Duration::from_secs(1));
    h.cx.run_until_parked();
    assert_eq!(
        h.state()["success_notice"],
        "Copied",
        "an older timer must not dismiss a newer success"
    );
    h.cx.advance_clock(Duration::from_secs(2));
    h.cx.run_until_parked();
    h.update(|window, cx, _| window.render_frame(cx));
    assert!(
        h.state()["success_notice"].is_null(),
        "copy feedback must dismiss after three seconds"
    );
    assert_eq!(h.state()["focused_rect"], before["focused_rect"]);
    h.export_format("png");
    h.ready();
    h.click("export-size");
    h.press("down");
    h.press("down");
    h.press("enter");
    h.ready();
    assert_ne!(h.state()["export_size"], "native");
    let revision = h.state()["export_preview"]["revision"].as_u64().unwrap();
    h.click("export-options");
    h.click("transparent");
    h.ready();
    assert!(h.state()["export_preview"]["revision"].as_u64().unwrap() > revision);
    assert!(h.state()["export_preview"]["error"].is_null());
    h.press("escape");
    assert_eq!(h.state()["export_options_open"], false);
    assert_eq!(h.state()["export_open"], true);
    h.click("export-options");
    let selected = h.state()["selected_tile"].as_u64().unwrap() as usize;
    h.click(("artwork", selected));
    assert_eq!(h.state()["export_options_open"], false);
    assert_eq!(h.state()["export_open"], true);
    h.capture("framed-transparent", false);
    h.shortcut("=");
    assert_eq!(
        h.state()["zoom"],
        before["zoom"],
        "export shortcuts must not move the artwork"
    );
    h.click("export-back");
    h.ready();
    assert_eq!(h.state()["focused_rect"], before["focused_rect"]);
    assert_eq!(h.state()["export_workspace"], false);
    let state = h.state();
    h.check("same-window-format-preview-copy-parameters-and-return",json!({"state":state,"real_renderer":true,"copied_formats":["embed","md","txt","json"],"system_save_dialog_exercised":false}));

    // Review the approved material and information hierarchy in the app's
    // Chinese dark appearance, using regenerated Grimoire artwork.
    h.press("escape");
    h.choose_style("grimoire");
    h.click("group-model");
    h.ready();
    h.focus("tier");
    h.shortcut(",");
    h.click("settings-tab-general");
    h.click("settings-language-zh");
    h.click("appearance");
    assert_eq!(h.state()["appearance"], "dark");
    h.click("panel-close");
    h.ready();
    h.capture("dark-focus", false);
    let first = h.state()["first_hit"].clone();
    hover_first_hit(h, &first);
    let name = format!("{}-dark-hover.png", h.key);
    h.cx.capture_screenshot(h.handle)
        .unwrap()
        .save(h.output.join(&name))
        .unwrap();
    h.artifacts.push(json!({"path":name}));
    h.click("focus-export");
    h.export_format("png");
    h.ready();
    h.capture("dark-png", false);
    h.export_format("md");
    h.ready();
    h.capture("dark-markdown", false);
    h.click("export-back");
    h.shortcut(",");
    h.click("appearance");
    h.click("appearance");
    assert_eq!(h.state()["appearance"], "light");
    h.click("panel-close");
    h.ready();
    let first = h.state()["first_hit"].clone();
    hover_first_hit(h, &first);
    let name = format!("{}-light-hover.png", h.key);
    h.cx.capture_screenshot(h.handle)
        .unwrap()
        .save(h.output.join(&name))
        .unwrap();
    h.artifacts.push(json!({"path":name}));
    h.check(
        "chinese-workspace-and-compact-hover-in-dark-and-light",
        json!({"real_renderer":true,"hover_title":first["t"]}),
    );
}

fn responsive(h: &mut Harness) {
    h.settle();
    h.ready();
    let inside = |h: &mut Harness, id: &'static str| {
        let id = if id == "panel" && h.state()["panel"] == "settings" {
            "creditia-panel"
        } else {
            id
        };
        let bounds = h.update(|window, _, _| window.find(id).bounds());
        assert!(
            f32::from(bounds.left()) >= 17. && f32::from(bounds.top()) >= 17.,
            "{id} escapes top/left: {bounds:?}"
        );
        assert!(
            f32::from(bounds.right()) <= 743. && f32::from(bounds.bottom()) <= 543.,
            "{id} escapes bottom/right: {bounds:?}"
        );
    };
    h.click("switch-project");
    inside(h, "panel");
    let (menu, trigger) = h.update(|window, _, _| {
        (
            window.find("panel").bounds().left(),
            window.find("switch-project").bounds().left(),
        )
    });
    assert_eq!(menu, trigger);
    h.capture("small-projects", false);
    h.click("switch-project");
    assert!(
        h.state()["panel"].is_null(),
        "the project trigger must also close its menu"
    );
    h.click("styles");
    inside(h, "panel");
    h.capture("small-styles", false);
    h.click("styles");
    assert!(
        h.state()["panel"].is_null(),
        "the style trigger must also close its menu"
    );
    h.click("settings");
    inside(h, "panel");
    for tab in [
        "settings-tab-general",
        "settings-tab-data",
        "settings-tab-about",
    ] {
        h.click(tab);
        inside(h, "panel");
    }
    h.capture("small-settings", false);
    h.click("panel-close");
    h.focus("tier");
    h.click("focus-layout");
    inside(h, "panel");
    h.capture("small-layout-menu", false);
    h.click(("variant", 0usize));
    h.ready();
    assert!(
        h.state()["panel"].is_null(),
        "choosing a layout closes its menu"
    );
    h.click("focus-layout");
    h.click("focus-layout");
    h.settle();
    assert!(
        h.state()["panel"].is_null(),
        "layout trigger closes its menu"
    );
    let before = h.state()["focused_rect"].clone();
    assert!(before["y"].as_f64().unwrap() + before["h"].as_f64().unwrap() <= 560. - 92. + 1.);
    h.capture("small-focus", false);
    h.click("focus-export");
    h.ready();
    h.update(|window, _, _| {
        let bounds = window.find("export-dock").bounds();
        assert!(bounds.size.width < px(760.));
        assert!(bounds.size.height < px(60.));
        for id in ["export-primary", "export-dock", "export-size"] {
            let b = window.find(id).bounds();
            assert!(
                b.left() >= px(0.)
                    && b.top() >= px(0.)
                    && b.right() <= px(760.)
                    && b.bottom() <= px(560.),
                "{id} is outside the export workspace: {b:?}"
            );
        }
    });
    assert_eq!(
        h.state()["focused_rect"],
        before,
        "opening Export must not resize the artwork"
    );
    h.capture("small-export", false);
    h.export_format("svg");
    h.ready();
    h.click("export-options");
    h.update(|window, _, _| {
        for id in [
            "export-primary",
            "export-size",
            "export-options-panel",
            "transparent",
            "font-mode",
        ] {
            let b = window.find(id).bounds();
            assert!(
                b.left() >= px(0.)
                    && b.top() >= px(0.)
                    && b.right() <= px(760.)
                    && b.bottom() <= px(560.),
                "{id} is outside the narrow SVG workspace: {b:?}"
            );
        }
    });
    h.capture("small-svg-options", false);
    h.press("escape");
    assert_eq!(h.state()["export_options_open"], false);
    assert_eq!(h.state()["export_open"], true);
    h.click("export-back");
    assert_eq!(h.state()["export_open"], false);
    h.click("focus-export");
    h.press("escape");
    assert_eq!(h.state()["focused"], true);
    assert_eq!(h.state()["export_open"], false);
    h.click("focus-export");
    h.export_format("md");
    h.ready();
    h.click("export-copy");
    let started = Instant::now();
    loop {
        h.cx.run_until_parked();
        h.update(|window, cx, _| window.render_frame(cx));
        if h.state()["operation_busy"] != true {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "copy Markdown did not finish"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let markdown = h.update(|_, cx, _| {
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap()
    });
    assert!(
        markdown.contains("<!-- creditia:start -->") && markdown.contains("<!-- creditia:end -->"),
        "Copy CREDITS.md must produce Markdown markers"
    );
    assert!(serde_json::from_str::<Value>(&markdown).is_err());
    h.click("export-back");
    h.click("styles");
    inside(h, "panel");
    h.capture("small-focus-styles", false);
    h.check("small-window-overlay-and-focus-bounds", json!({"width":760,"height":560,"focused_rect":before,"export_does_not_resize_artwork":true,"all_settings_tabs":true}));
}

fn card_layout(h: &mut Harness) {
    for mode in ["wall", "roam"] {
        h.click(mode);
        h.ready();
        h.focus("tier");
        h.click("focus-close-top");
        h.ready();
        let before = h.state();
        let index = before["selected_tile"].as_u64().unwrap() as usize;
        h.update(|window, cx, _| window.hover(("artwork", index), cx));
        h.click(("tile-next-layout", index));
        h.ready();
        let after = h.state();
        assert_eq!(after["focused"], false);
        assert_eq!(after["selected_piece"], before["selected_piece"]);
        assert_eq!(after["view"], mode);
        assert_ne!(
            after["selected_variant"], before["selected_variant"],
            "the card arrow must change its layout"
        );
        h.capture(&format!("{mode}-card-layout"), false);
        h.shortcut("e");
        h.ready();
        assert_eq!(h.state()["focused"], false);
        assert_eq!(h.state()["selected_rect"], after["selected_rect"]);
        let export_state = h.state();
        let export_index = export_state["selected_tile"].as_u64().unwrap() as usize;
        h.update(|window, cx, _| window.hover(("artwork", export_index), cx));
        h.capture(&format!("{mode}-card-export"), false);
        h.update(|window, cx, _| {
            window.render_frame(cx);
            assert!(window.find("export-dock").visible());
            for control in ["tile-previous-layout", "tile-next-layout", "tile-export"] {
                assert!(
                    window.find_all((control, export_index)).is_empty(),
                    "export retained a duplicate corner tool: {control}; state={export_state}"
                );
            }
        });
        h.press("escape");
        assert_eq!(h.state()["selected_rect"], after["selected_rect"]);
    }
    h.check(
        "card-layout-and-export-keep-the-current-artwork",
        json!({"wall":true,"roam":true,"export_hides_corner_tools":true,"export_keeps_artwork_position":true}),
    );
}

fn overlay_motion(h: &mut Harness) {
    h.click("settings");
    h.click("reduced-motion");
    assert_eq!(h.state()["reduced_motion"], false);
    h.click("panel-close");
    h.settle();
    h.click("styles");
    h.click("styles");
    h.click("styles");
    h.settle();
    assert_eq!(h.state()["panel"], "styles");
    h.click("pieces");
    h.settle();
    assert_eq!(h.state()["panel"], "pieces");
    h.press("escape");
    h.settle();
    assert!(
        h.state()["panel"].is_null(),
        "Escape did not close the overlay: {}",
        h.state()
    );
    h.check("animated-overlays-reverse-and-switch-during-transitions",json!({"reduced_motion":false,"reopen_during_close":true,"switch_panel":true,"escape":true}));
}

pub fn run() {
    let input = PathBuf::from(
        std::env::var("CREDITIA_VISUAL_FIXTURES").expect("canonical fixture directory"),
    );
    let output = PathBuf::from(
        std::env::var("CREDITIA_RUNTIME_OUTPUT")
            .unwrap_or_else(|_| "evidence/native-runtime".into()),
    );
    fs::create_dir_all(&output).unwrap();
    let fixture: Value =
        serde_json::from_slice(&fs::read(input.join("minimal-light-en.json")).unwrap()).unwrap();
    let source: Value =
        serde_json::from_slice(&fs::read(input.join("fixture-manifest.json")).unwrap()).unwrap();
    let mut cases = Vec::new();
    let scenarios: [(&str, fn(&mut Harness)); 11] = [
        ("runtime-layouts", layouts),
        ("runtime-style-trial", trial_restore),
        ("runtime-style-apply", apply_style),
        ("runtime-groups", groups),
        ("runtime-preferences", preferences),
        ("runtime-spatial", spatial),
        ("runtime-responsive", responsive),
        ("runtime-card-layout", card_layout),
        ("runtime-overlay-motion", overlay_motion),
        ("runtime-reviewed-interactions", reviewed_interactions),
        ("runtime-export-workspace", export_workspace),
    ];
    for (key, scenario) in scenarios {
        if std::env::var("CREDITIA_RUNTIME_CASE")
            .ok()
            .is_some_and(|selected| selected != key)
        {
            continue;
        }
        println!("native runtime case: {key}");
        let created = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Harness::new(
                fixture["raw"].clone(),
                fixture["catalog"].clone(),
                &output,
                key,
                if key == "runtime-responsive" {
                    (760., 560.)
                } else {
                    (1440., 1000.)
                },
                Preferences {
                    language: Language::En,
                    appearance: Appearance::Light,
                    named: false,
                    export_format: if key == "runtime-reviewed-interactions" {
                        "gif"
                    } else {
                        "png"
                    }
                    .into(),
                    ..Default::default()
                },
            )
        }));
        let mut harness = match created {
            Ok(value) => value,
            Err(error) => {
                cases.push(json!({"key":key,"status":"failed","error":panic_message(error),"checks":[],"artifacts":[]}));
                continue;
            }
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            harness.setup();
            scenario(&mut harness);
        }));
        cases.push(match result {
            Ok(()) => { let state=harness.state(); json!({"key":key,"status":"passed","state":state,"checks":harness.checks,"feature_checks":harness.feature_checks,"artifacts":harness.artifacts}) },
            Err(error) => harness.failure(panic_message(error)),
        });
    }
    let complete = cases.iter().all(|case| case["status"] == "passed");
    let manifest = json!({"schema_version":1,"kind":"native-ui","provenance":{"candidate_sha":std::env::var("GITHUB_SHA").ok(),"environment":{"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"renderer":if cfg!(target_os="macos"){"Metal"}else{"WGPU"}},"fixture":source["fixture"],"profile":"real-shared-renderer-inputs"},"method":"Production benchmark constructor with raw archive only; native pointer/keyboard actions regenerate real shared-renderer previews. No precomputed artwork is supplied. File persistence, project services and animation playback are outside this suite.","cases":cases,"complete":complete});
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(
        complete,
        "Production runtime input/renderer acceptance failed"
    );
}
fn panic_message(error: Box<dyn std::any::Any + Send>) -> String {
    error
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| error.downcast_ref::<&str>().map(|value| value.to_string()))
        .unwrap_or_else(|| "Native runtime case panicked".into())
}
