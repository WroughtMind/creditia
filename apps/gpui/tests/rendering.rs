//! Production-view pixel/input gate. Uses Metal/WGPU, never a simulated image.
//! Run explicitly on the main thread: cargo test --features visual-test --test rendering.
fn main() {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    native::run();
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    panic!("Native pixel validation requires a supported Metal or WGPU headless renderer");
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod native {
    use creditia_gpui::{
        Appearance,
        app::{Creditia, Fixture},
    };
    use gpui_kit::test::{TestQueryExt, TestWindowExt};
    use gpui_kit::{
        AnyWindowHandle, AppContext, Bounds, Entity, HeadlessAppContext, WindowBounds,
        WindowOptions, assets::Assets, px, size,
    };
    use serde_json::{Value, json};
    use std::{
        collections::HashSet,
        fs,
        path::{Path, PathBuf},
        sync::Arc,
        time::{Duration, Instant},
    };

    fn wait_assets(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, view: &Entity<Creditia>) {
        let start = Instant::now();
        loop {
            cx.run_until_parked();
            let ready = cx
                .update_window(handle, |_, window, cx| {
                    window.render_frame(cx);
                    view.update(cx, |view, cx| view.fixture_images_ready(window, cx))
                })
                .expect("fixture window exists");
            if ready {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(45),
                "production artwork assets did not decode"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn case(
        input: &Path,
        theme: Appearance,
        key: &str,
        output: &Path,
        interact: bool,
        state: Option<&str>,
    ) -> Value {
        let mut fixture: Fixture =
            serde_json::from_slice(&fs::read(input).expect("fixture exists"))
                .expect("valid native fixture");
        fixture.appearance = theme;
        fixture.reduced_motion = true;
        if let Some(state) = state {
            fixture.state = state.into();
            fixture.message = "读取失败，请重试 / Read failed; retry is available".into();
            fixture.progress = json!({"stage":"logs","done":2,"total":5});
        }
        let layout_choice = if interact {
            let style = fixture.catalog["styles"]
                .as_array()
                .unwrap()
                .iter()
                .find(|style| style["id"] == "minimal-light")
                .unwrap();
            let pieces = style["pieces"].as_array().unwrap();
            let piece_index = pieces
                .iter()
                .position(|piece| piece["id"] == "tier")
                .unwrap();
            let piece = &pieces[piece_index];
            let variants = piece["variants"].as_array().unwrap();
            let current = piece["defaultVariant"]
                .as_str()
                .or_else(|| variants[0]["id"].as_str())
                .unwrap();
            let (variant_index, variant) = variants
                .iter()
                .enumerate()
                .find(|(_, variant)| variant["id"] != current)
                .unwrap();
            Some((
                piece_index,
                variant_index,
                variant["id"].as_str().unwrap().to_owned(),
            ))
        } else {
            None
        };
        let mut cx = HeadlessAppContext::with_platform(
            gpui_kit::platform::current_platform(true).text_system(),
            Arc::new(Assets),
            gpui_kit::platform::current_headless_renderer,
        );
        cx.update(gpui_kit::init);
        cx.update(|cx| {
            creditia_gpui::app::init_fonts(cx).expect("the declared UI font files load")
        });
        cx.update(|cx| cx.set_reduce_motion(true));
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
                    |window, cx| cx.new(|cx| Creditia::fixture(fixture, window, cx)),
                )
            })
            .expect("production window opens");
        wait_assets(&mut cx, handle, &view);
        let mut checks = vec![];
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("wall").visible());
            assert!(window.find("roam").visible());
            assert!(window.find("copy").visible());
            match state {
                Some("empty") => assert!(window.find("first-project").visible()),
                Some("loading") => assert_eq!(view.read(cx).diagnostic_state()["loading"], true),
                Some("error") => assert!(window.find("retry").visible()),
                Some("analyzing") => assert!(window.find("cancel-analysis").visible()),
                Some("exporting") => assert!(window.find("cancel-export").visible()),
                _ => {}
            }
        })
        .unwrap();
        checks.push(json!({"id":"production-controls-visible","passed":true}));
        if let Some(state) = state {
            checks.push(
                json!({"id":"production-empty-error-task-state","passed":true,"state":state}),
            );
        }
        let image = cx.capture_screenshot(handle).expect("real GPU capture");
        let colors = image
            .as_raw()
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]])
            .collect::<HashSet<_>>();
        assert!(
            colors.len() > 64,
            "native window is blank or missing artwork"
        );
        let dimensions = image.dimensions();
        let filename = format!("{key}.png");
        image
            .save(output.join(&filename))
            .expect("save native pixels");
        checks
            .push(json!({"id":"real-renderer-pixels","passed":true,"unique_colors":colors.len()}));
        let mut artifacts = vec![json!({"path":filename})];
        if interact {
            cx.update_window(handle, |_, window, cx| {
                window.click("roam", cx);
                assert_eq!(view.read(cx).diagnostic_state()["view"], "roam");
                let before = view.read(cx).diagnostic_state()["zoom"].as_f64().unwrap();
                window.click("zoom-in", cx);
                assert!(view.read(cx).diagnostic_state()["zoom"].as_f64().unwrap() > before);
                window.render_frame(cx);
            })
            .unwrap();
            let name = format!("{key}-roam.png");
            cx.capture_screenshot(handle)
                .unwrap()
                .save(output.join(&name))
                .unwrap();
            artifacts.push(json!({"path":name}));
            cx.update_window(handle, |_, window, cx| {
                window.click("wall", cx);
                assert_eq!(view.read(cx).diagnostic_state()["view"], "wall");
                window.click("settings", cx);
                assert!(window.find("names-settings").visible());
                let before = view.read(cx).diagnostic_state()["named"].as_bool().unwrap();
                window.click("names-settings", cx);
                assert_eq!(view.read(cx).diagnostic_state()["named"], !before);
                window.render_frame(cx);
            })
            .unwrap();
            checks.push(json!({"id":"native-input-roam-zoom-settings","passed":true}));
            let (piece_index, variant_index, variant) = layout_choice.unwrap();
            cx.update_window(handle, |_, window, cx| {
                window.click("panel-close", cx);
                window.click("pieces", cx);
                window.click(("piece", piece_index), cx);
                assert_eq!(view.read(cx).diagnostic_state()["selected_piece"], "tier");
                assert_eq!(view.read(cx).diagnostic_state()["focused"], true);
                window.render_frame(cx);
            })
            .unwrap();
            wait_assets(&mut cx, handle, &view);
            let before = cx.capture_screenshot(handle).unwrap();
            cx.update_window(handle, |_, window, cx| {
                window.click("layouts", cx);
                window.click(("variant", variant_index), cx);
                assert_eq!(
                    view.read(cx).diagnostic_state()["selected_variant"],
                    variant.as_str()
                );
                window.render_frame(cx);
            })
            .unwrap();
            wait_assets(&mut cx, handle, &view);
            let after = cx.capture_screenshot(handle).unwrap();
            assert!(
                before != after,
                "changing a layout must change the focused artwork pixels"
            );
            let name = format!("{key}-layout.png");
            after.save(output.join(&name)).unwrap();
            artifacts.push(json!({"path":name}));
            checks.push(json!({"id":"native-artwork-layout-selection","passed":true,"piece":"tier","variant":variant}));
        }
        json!({"key":key,"status":"passed","fixture_state":state.unwrap_or("ready"),"window":{"width":1440,"height":1000,"pixel_width":dimensions.0,"pixel_height":dimensions.1,"dpi_scale":dimensions.0 as f64/1440.0},"checks":checks,"artifacts":artifacts})
    }

    pub fn run() {
        let fixtures = PathBuf::from(
            std::env::var("CREDITIA_VISUAL_FIXTURES").expect("set CREDITIA_VISUAL_FIXTURES"),
        );
        let output = PathBuf::from(
            std::env::var("CREDITIA_VISUAL_OUTPUT").unwrap_or_else(|_| "evidence/native".into()),
        );
        fs::create_dir_all(&output).unwrap();
        let fixture_manifest: Value =
            serde_json::from_slice(&fs::read(fixtures.join("fixture-manifest.json")).unwrap())
                .unwrap();
        let mut cases = vec![];
        for entry in fixture_manifest["files"].as_array().unwrap() {
            for (theme, label) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
                let key = format!(
                    "{}-{}-{label}-wall",
                    entry["style"].as_str().unwrap(),
                    entry["language"].as_str().unwrap()
                );
                println!("native case: {key}");
                let input = fixtures.join(entry["path"].as_str().unwrap());
                let interact = entry["style"] == "minimal-light"
                    && entry["language"] == "en"
                    && label == "light";
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    case(&input, theme, &key, &output, interact, None)
                }));
                cases.push(match result {Ok(case)=>case,Err(error)=>{let message=error.downcast_ref::<String>().cloned().or_else(||error.downcast_ref::<&str>().map(|s|s.to_string())).unwrap_or_else(||"Native case panicked".into());json!({"key":key,"status":"failed","error":message,"checks":[],"artifacts":[]})}});
            }
        }
        for state in ["empty", "loading", "error", "analyzing", "exporting"] {
            for language in ["zh", "en"] {
                for (theme, label) in [(Appearance::Light, "light"), (Appearance::Dark, "dark")] {
                    let key = format!("state-{state}-{language}-{label}");
                    println!("native case: {key}");
                    let input = fixtures.join(format!("minimal-light-{language}.json"));
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        case(&input, theme, &key, &output, false, Some(state))
                    }));
                    cases.push(match result{Ok(case)=>case,Err(error)=>{let message=error.downcast_ref::<String>().cloned().or_else(||error.downcast_ref::<&str>().map(|s|s.to_string())).unwrap_or_else(||"Native state case panicked".into());json!({"key":key,"status":"failed","error":message,"checks":[],"artifacts":[]})}});
                }
            }
        }
        let complete = cases.iter().all(|c| c["status"] == "passed");
        let report = json!({"schema_version":1,"kind":"native-ui","provenance":{"candidate_sha":std::env::var("GITHUB_SHA").ok(),"environment":{"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"renderer":if cfg!(target_os="macos"){"Metal"}else{"WGPU"}},"fixture":fixture_manifest["fixture"],"profile":fixture_manifest["profile"]},"cases":cases,"complete":complete});
        fs::write(
            output.join("manifest.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        assert!(
            complete,
            "one or more native visual/input cases failed; inspect native manifest and screenshots"
        );
    }
}
