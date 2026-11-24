//! Capture the production shell's secondary surfaces through native input.
use creditia_gpui::app::{Creditia, Fixture};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, HeadlessAppContext, WindowBounds, WindowOptions,
    accesskit::Role, assets::Assets, px, size,
};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

fn wait_assets(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, view: &Entity<Creditia>) {
    let started = Instant::now();
    loop {
        cx.run_until_parked();
        let ready = cx
            .update_window(handle, |_, window, cx| {
                view.update(cx, |_, cx| cx.notify());
                window.render_frame(cx);
                view.update(cx, |view, cx| {
                    view.fixture_images_ready(window, cx)
                        && view.fixture_panel_images_ready(window, cx)
                })
            })
            .unwrap();
        if ready {
            return;
        }
        assert!(
            started.elapsed() < Duration::from_secs(90),
            "surface previews did not decode: {:?}",
            cx.update(|cx| view.read(cx).diagnostic_state())
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn capture(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, output: &Path, key: &str) {
    cx.capture_screenshot(handle)
        .unwrap()
        .save(output.join(format!("{key}.png")))
        .unwrap();
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let input = PathBuf::from(args.get(1).expect("fixture JSON"));
    let output = PathBuf::from(args.get(2).expect("capture directory"));
    fs::create_dir_all(&output).unwrap();
    let mut fixture: Fixture = serde_json::from_slice(&fs::read(&input).unwrap()).unwrap();
    fixture.reduced_motion = true;
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);
    cx.update(|cx| {
        creditia_gpui::app::init_fonts(cx).unwrap();
        creditia_gpui::app::bind_keys(cx);
        cx.set_reduce_motion(true);
    });
    let (handle, view) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(1100.), px(800.)),
                    })),
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| Creditia::fixture(fixture, window, cx)),
            )
        })
        .unwrap();
    wait_assets(&mut cx, handle, &view);

    cx.update_window(handle, |_, window, cx| window.click("settings", cx))
        .unwrap();
    wait_assets(&mut cx, handle, &view);
    capture(&mut cx, handle, &output, "settings");
    cx.update_window(handle, |_, window, cx| {
        let switch = window.find("names-settings");
        let original = view.read(cx).diagnostic_state()["named"].as_bool().unwrap();
        assert_eq!(switch.role(), Some(Role::Switch));
        assert_eq!(switch.checked(), Some(original));
        window.click("names-settings", cx);
        window.render_frame(cx);
        assert_eq!(window.find("names-settings").checked(), Some(!original));
        for _ in 0..20 {
            if window.find("names-settings").focused() == Some(true) {
                break;
            }
            window.press("tab", cx);
            window.render_frame(cx);
        }
        assert_eq!(window.find("names-settings").focused(), Some(true));
        window.press("space", cx);
        window.render_frame(cx);
        assert_eq!(view.read(cx).diagnostic_state()["named"], original);
        window.click("panel-close", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("switch-project", cx);
    })
    .unwrap();
    wait_assets(&mut cx, handle, &view);
    capture(&mut cx, handle, &output, "projects");
    cx.update_window(handle, |_, window, cx| {
        window.click(("project-more", 0usize), cx);
        window.render_frame(cx);
        window.click("row-settings", cx);
        window.render_frame(cx);
        assert_eq!(view.read(cx).diagnostic_state()["panel"], "project");
    })
    .unwrap();
    wait_assets(&mut cx, handle, &view);
    capture(&mut cx, handle, &output, "project");

    cx.update_window(handle, |_, window, cx| {
        window.click("panel-close", cx);
        window.render_frame(cx);
        window.click("styles", cx);
    })
    .unwrap();
    wait_assets(&mut cx, handle, &view);
    capture(&mut cx, handle, &output, "styles");

    cx.update_window(handle, |_, window, cx| {
        window.click("styles", cx);
        window.render_frame(cx);
        window.click("pieces", cx);
        window.render_frame(cx);
        window.click(("piece", 0usize), cx);
    })
    .unwrap();
    wait_assets(&mut cx, handle, &view);
    cx.update_window(handle, |_, window, cx| {
        let zoom = view.read(cx).diagnostic_state()["zoom"].clone();
        let width = window.find("zoom-reset").bounds().size.width;
        window.click("zoom-in", cx);
        window.render_frame(cx);
        assert!(view.read(cx).diagnostic_state()["zoom"].as_f64() > zoom.as_f64());
        assert_eq!(window.find("zoom-reset").bounds().size.width, width);
        window.click("zoom-out", cx);
        window.render_frame(cx);
        window.click("zoom-reset", cx);
        window.render_frame(cx);
        assert_eq!(view.read(cx).diagnostic_state()["zoom"], 1.);
        window.click("focus-export", cx);
        window.render_frame(cx);
        assert_eq!(view.read(cx).diagnostic_state()["export_workspace"], true);
    })
    .unwrap();
    wait_assets(&mut cx, handle, &view);
    cx.update_window(handle, |_, window, cx| {
        let transparent = window.find("transparent");
        assert_eq!(transparent.role(), Some(Role::CheckBox));
        let original = transparent.checked().unwrap();
        window.click("transparent", cx);
        window.render_frame(cx);
        assert_eq!(window.find("transparent").checked(), Some(!original));
        window.click("export-size", cx);
        window.render_frame(cx);
        window.press("down", cx);
        window.press("down", cx);
        window.press("enter", cx);
        window.render_frame(cx);
        assert_ne!(view.read(cx).diagnostic_state()["export_size"], "native");
        assert!(window.find("export-size").label().unwrap().contains('×'));
    })
    .unwrap();
    wait_assets(&mut cx, handle, &view);
    capture(&mut cx, handle, &output, "export");
    cx.update_window(handle, |_, window, cx| {
        window.click("export-format", cx);
        window.render_frame(cx);
        let png = window.find("export-format-png");
        let embed = window.find("export-format-embed");
        assert_eq!(png.checked(), Some(true));
        assert!(embed.visible());
        assert_eq!(png.bounds().size.width, embed.bounds().size.width);
    })
    .unwrap();
    capture(&mut cx, handle, &output, "export-formats");
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "kind": "native-optical-surfaces",
            "platform": std::env::consts::OS,
            "renderer": if cfg!(target_os = "macos") { "Metal" } else { "WGPU" },
            "fixture": input.file_name().unwrap(),
            "captures": ["settings", "projects", "styles", "project", "export", "export-formats"],
            "checked": ["switch pointer and keyboard activation", "project row menu",
                        "focus zoom", "export format layout", "size selection", "transparency checkbox"],
        }))
        .unwrap(),
    )
    .unwrap();
}
