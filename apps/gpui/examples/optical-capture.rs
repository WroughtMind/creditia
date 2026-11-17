//! Focused production-view captures for reviewing the shell's secondary surfaces.
use creditia_gpui::app::{Creditia, Fixture};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Bounds, HeadlessAppContext, WindowBounds, WindowOptions, assets::Assets, px, size,
};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let input = PathBuf::from(args.get(1).expect("fixture JSON"));
    let output = PathBuf::from(args.get(2).expect("capture directory"));
    fs::create_dir_all(&output).unwrap();
    let mut fixture: Fixture = serde_json::from_slice(&fs::read(input).unwrap()).unwrap();
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
    let started = Instant::now();
    loop {
        cx.run_until_parked();
        let ready = cx
            .update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                view.update(cx, |view, cx| view.fixture_images_ready(window, cx))
            })
            .unwrap();
        if ready {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(90),
            "initial artwork decode timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut captures = vec![];
    for (key, control) in [
        ("settings", "settings"),
        ("projects", "switch-project"),
        ("styles", "styles"),
        ("project", "project-settings"),
    ] {
        println!("capture: {key}");
        cx.update_window(handle, |_, window, cx| {
            window.click(control, cx);
            window.render_frame(cx);
            assert!(window.find("panel-close").visible());
        })
        .unwrap();
        let settled = Instant::now();
        loop {
            let ready = cx
                .update_window(handle, |_, window, cx| {
                    window.render_frame(cx);
                    view.update(cx, |view, cx| view.fixture_images_ready(window, cx))
                })
                .unwrap();
            if ready {
                break;
            }
            assert!(
                settled.elapsed() < Duration::from_secs(30),
                "UI previews failed to settle"
            );
            cx.run_until_parked();
        }
        cx.capture_screenshot(handle)
            .unwrap()
            .save(output.join(format!("{key}.png")))
            .unwrap();
        captures.push(key);
        cx.update_window(handle, |_, window, cx| {
            window.click("panel-close", cx);
            window.render_frame(cx);
        })
        .unwrap();
    }
    cx.update_window(handle, |_, window, cx| {
        window.press("ctrl-e", cx);
        window.render_frame(cx);
    })
    .unwrap();
    // Reduced motion makes the actual export dock settle on the next render.
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.update_window(handle, |_, window, _| {
        assert!(window.find("close-export-dock").visible())
    })
    .unwrap();
    cx.capture_screenshot(handle)
        .unwrap()
        .save(output.join("export.png"))
        .unwrap();
    captures.push("export");
    cx.update_window(handle, |_, window, cx| {
        window.click("close-export-dock", cx);
        window.click("settings", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        let original = view.read(cx).diagnostic_state()["named"].as_bool().unwrap();
        window.click("names-settings", cx);
        assert_eq!(view.read(cx).diagnostic_state()["named"], !original);
        window.render_frame(cx);
        // Kit buttons preserve keyboard focus on pointer activation.
        // Tab from the panel close control to the contributor switch.
        for _ in 0..4 {
            window.press("tab", cx);
        }
        window.press("space", cx);
        assert_eq!(view.read(cx).diagnostic_state()["named"], original);
        window.render_frame(cx);
    })
    .unwrap();
    fs::write(output.join("manifest.json"), serde_json::to_vec_pretty(&json!({
        "kind":"native-optical-surfaces", "platform":std::env::consts::OS,
        "fixture":"synthetic catnap", "captures":captures,
        "scope":"production pixels and surface navigation; not persistence or export-file validation"
    })).unwrap()).unwrap();
}
