//! Real production input/preview integration: no pre-rendered fixture artwork.
//! Focus autoplay scheduling uses production input; frame playback, persistence and analysis have separate gates.
use creditia_gpui::app::Creditia;
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, ElementId, Entity, HeadlessAppContext, InputEvent,
    MouseMoveEvent, Window, WindowBounds, WindowOptions, assets::Assets, point, px, size,
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
    fn new(raw: Value, catalog: Value, output: &Path, key: &str) -> Self {
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
                            size: size(px(1440.), px(1000.)),
                        })),
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    cx,
                    |window, cx| cx.new(|cx| Creditia::benchmark(raw, window, cx)),
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
    fn ready(&mut self) {
        let started = Instant::now();
        loop {
            self.cx.run_until_parked();
            let (ready, state) = self.update(|window, cx, view| {
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
    fn autoplay_scheduled(&mut self, label: &str) -> Value {
        let started = Instant::now();
        loop {
            let (ready, state) = self.update(|window, cx, view| {
                window.render_frame(cx);
                (
                    view.update(cx, |view, cx| view.fixture_images_ready(window, cx)),
                    view.read(cx).diagnostic_state(),
                )
            });
            assert!(
                state["error"].is_null(),
                "{label}: renderer failed: {state}"
            );
            if state["animation_busy"] == true {
                assert_eq!(state["focused"], true);
                assert_eq!(state["focus_autoplay_pending"], false);
                return state;
            }
            assert!(
                !ready,
                "{label}: a decoded focused artwork did not schedule autoplay: {state}"
            );
            assert!(
                started.elapsed() < Duration::from_secs(90),
                "{label}: {state}"
            );
            // Do not poll the long-lived blocking producer on TestScheduler's
            // synchronous background executor. Assert scheduling, then cancel
            // via real input before advancing tasks; Playback has its own tests.
            self.cx
                .background_executor()
                .dispatcher()
                .as_test()
                .expect("headless test dispatcher")
                .tick(false);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn settle(&mut self) {
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(220) {
            self.cx.run_until_parked();
            self.update(|window, cx, _| window.render_frame(cx));
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
            window.hover("styles", cx);
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
    fn failure(&mut self, error: String) -> Value {
        let state = self.state();
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
    h.click("layouts");
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
    let settled = h.capture("settled-third-layout", true);
    h.click("layouts");
    h.click(("variant", variants.len() - 1));
    // Intentionally issue new selections before the background renderer settles.
    h.press("right");
    h.press("right");
    h.press("right");
    h.ready();
    assert_eq!(h.state()["selected_variant"], variants[2]);
    let rapid = h.capture("rapid-final-layout", true);
    assert_eq!(
        settled, rapid,
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
    h.click("language");
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
    h.click("zoom-in");
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
    h.click("zoom-in");
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
    h.update(|window, _, _| assert!(window.find("close-export-dock").visible()));
    h.capture("export-dock", false);
    h.click("close-export-dock");
    assert_eq!(h.state()["export_open"], false);
    let state = h.state();
    h.check("actual-hover-roam-drag-zoom-and-export-dock",json!({"hover_title":first["t"],"drag_before":before,"drag_after":after,"state":state,"export_file_written":false}));
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

fn focus_autoplay(h: &mut Harness) {
    h.focus("tier");
    let reduced = h.state();
    assert!(reduced["animation"].is_null());
    assert_eq!(reduced["animation_busy"], false);
    assert_eq!(reduced["focus_autoplay_pending"], false);
    h.press("escape");
    h.click("settings");
    h.click("reduced-motion");
    h.click("panel-close");
    assert_eq!(h.state()["reduced_motion"], false);
    // This activates only TestPlatform's hidden window, never an OS window.
    h.update(|window, _, _| window.activate_window());
    h.settle();
    h.update(|window, _, _| assert!(window.is_window_active()));
    assert_eq!(
        h.state()["animation_busy"],
        false,
        "the exhibition wall never autoplays"
    );
    let index = h.style("minimal-light")["pieces"]
        .as_array()
        .unwrap()
        .iter()
        .position(|piece| piece["id"] == "tier")
        .unwrap();
    h.click("pieces");
    h.click(("piece", index));
    // Change the viewport while the entrance/static preview is still pending.
    h.click("zoom-in");
    h.update(|window, cx, _| window.drag(point(px(700.), px(450.)), point(px(745.), px(475.)), cx));
    let positioned = h.state();
    assert!(positioned["zoom"].as_f64().unwrap() > 1.);
    let opened = h.autoplay_scheduled("opening a focused artwork");
    for key in ["zoom", "pan_x", "pan_y"] {
        assert_eq!(opened[key], positioned[key], "autoplay changed {key}");
    }
    h.capture("static-preview-before-autoplay-decoding", false);
    // Cancel before TestScheduler polls the producer, just as a user can switch
    // while the real worker is waiting for its first render response.
    h.click("next-piece");
    assert_eq!(
        h.state()["animation_busy"],
        false,
        "switching cancels the old producer"
    );
    let switched = h.autoplay_scheduled("switching focused artwork");
    assert_ne!(switched["selected_id"], opened["selected_id"]);
    // The next wall item can have no variants (for example people). Return to
    // the known tier selection before exercising rapid layout replacements.
    h.click("pieces");
    h.click(("piece", index));
    h.autoplay_scheduled("returning to an artwork with layouts");
    h.press("right");
    h.press("right");
    assert_eq!(
        h.state()["animation_busy"],
        false,
        "layout replacement cancels the old producer"
    );
    let replaced = h.autoplay_scheduled("only the latest focused layout");
    // Change style through the real modal without the helper's task polling.
    let style = h.catalog["styles"]
        .as_array()
        .unwrap()
        .iter()
        .position(|style| style["id"] == "ink")
        .unwrap();
    h.click("styles");
    h.click(("style", style));
    assert_eq!(
        h.state()["animation_busy"],
        false,
        "style replacement cancels the old producer"
    );
    let restyled = h.autoplay_scheduled("focused style replacement");
    assert_eq!(restyled["style"], "ink");
    h.press("escape");
    h.settle();
    let closed = h.state();
    assert_eq!(closed["focused"], false);
    assert_eq!(closed["focus_autoplay_pending"], false);
    assert_eq!(closed["animation_busy"], false);
    assert!(closed["animation"].is_null());
    h.check(
        "focused-autoplay-scheduling-cancellation-and-viewport",
        json!({
            "reduced_motion":reduced,"positioned":positioned,"opened":opened,
            "switched":switched,"replaced":replaced,"restyled":restyled,"closed":closed,
            "player_frames_executed":false,
        }),
    );
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
    let scenarios: [(&str, fn(&mut Harness)); 7] = [
        ("runtime-layouts", layouts),
        ("runtime-style-trial", trial_restore),
        ("runtime-style-apply", apply_style),
        ("runtime-groups", groups),
        ("runtime-preferences", preferences),
        ("runtime-spatial", spatial),
        ("runtime-focus-autoplay", focus_autoplay),
    ];
    let filter = std::env::args()
        .skip(1)
        .find(|argument| !argument.starts_with('-'));
    for (key, scenario) in scenarios {
        if filter
            .as_deref()
            .is_some_and(|filter| !key.contains(filter))
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
    let complete = !cases.is_empty() && cases.iter().all(|case| case["status"] == "passed");
    let manifest = json!({"schema_version":1,"kind":"native-ui","provenance":{"candidate_sha":std::env::var("GITHUB_SHA").ok(),"environment":{"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"renderer":if cfg!(target_os="macos"){"Metal"}else{"WGPU"}},"fixture":source["fixture"],"profile":"real-shared-renderer-inputs","filter":filter},"method":"Production benchmark constructor with raw archive only; native pointer/keyboard actions regenerate real shared-renderer previews. No precomputed artwork is supplied. Focus autoplay checks scheduling, cancellation, reduced motion and viewport preservation before advancing the blocking player producer. Animation frames, file persistence and project services are outside this suite.","cases":cases,"complete":complete});
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
