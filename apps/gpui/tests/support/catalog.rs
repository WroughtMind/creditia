//! Every desktop artwork/layout is selected and painted by the production view.
use creditia_gpui::app::{Creditia, Fixture};
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, HeadlessAppContext, WindowBounds, WindowOptions,
    assets::Assets, px, size,
};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

fn decoded(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, view: &Entity<Creditia>) {
    let started = Instant::now();
    loop {
        cx.run_until_parked();
        if cx
            .update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                view.update(cx, |view, cx| view.fixture_images_ready(window, cx))
            })
            .unwrap()
        {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "selected native artwork did not decode"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub fn run() {
    let input =
        PathBuf::from(std::env::var("CREDITIA_NATIVE_CATALOG").expect("catalog input directory"));
    let output = PathBuf::from(
        std::env::var("CREDITIA_NATIVE_CATALOG_CAPTURE").expect("catalog capture directory"),
    );
    fs::create_dir_all(&output).unwrap();
    let inputs: Value =
        serde_json::from_slice(&fs::read(input.join("catalog-inputs.json")).unwrap()).unwrap();
    let mut rows = Vec::new();
    for batch in inputs["batches"].as_array().unwrap() {
        let fixture: Fixture =
            serde_json::from_slice(&fs::read(input.join(batch["path"].as_str().unwrap())).unwrap())
                .unwrap();
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
                    |window, cx| cx.new(|cx| Creditia::fixture(fixture, window, cx)),
                )
            })
            .unwrap();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        let mut batch_failed = false;
        for row in batch["rows"].as_array().unwrap() {
            let key = row["row"].as_str().unwrap();
            if batch_failed {
                rows.push(json!({"row":key,"status":"not_run","error":"Previous input failure invalidated this style window"}));
                continue;
            }
            println!("native catalog: {key}");
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                cx.update_window(handle, |_, window, cx| {
                    window.click("pieces", cx);
                    assert_eq!(view.read(cx).diagnostic_state()["panel"], "pieces", "the Artworks control must open its own selector without clicking through to the wall");
                    window.click(("piece", row["piece_index"].as_u64().unwrap() as usize), cx);
                    assert_eq!(view.read(cx).diagnostic_state()["selected_piece"], row["piece"], "the piece selector must preserve the requested selection");
                    assert_eq!(view.read(cx).diagnostic_state()["focused"], true);
                    if let Some(index) = row["variant_index"].as_u64() {
                        window.click("layouts", cx);
                        assert_eq!(view.read(cx).diagnostic_state()["panel"], "layouts");
                        window.click(("variant", index as usize), cx);
                    }
                    window.render_frame(cx);
                    let actual = view.read(cx).diagnostic_state();
                    assert_eq!(actual["style"], row["style"]);
                    assert_eq!(actual["selected_piece"], row["piece"]);
                    assert_eq!(actual["selected_variant"], row["variant"]);
                    assert_eq!(actual["focused"], true);
                })
                .unwrap();
                decoded(&mut cx, handle, &view);
                let (actual, bounds) = cx
                    .update_window(handle, |_, window, cx| {
                        window.render_frame(cx);
                        let actual = view.read(cx).diagnostic_state();
                        let index = actual["selected_tile"].as_u64().unwrap() as usize;
                        let picture = window.find(("artwork", index));
                        assert!(
                            picture.visible(),
                            "selected artwork must be inside the actual native viewport"
                        );
                        (actual, picture.bounds())
                    })
                    .unwrap();
                let image = cx
                    .capture_screenshot(handle)
                    .expect("real WGPU catalog capture");
                let scale = image.width() as f32 / 1440.;
                let (left, top, width, height): (f32, f32, f32, f32) = (
                    bounds.origin.x.into(),
                    bounds.origin.y.into(),
                    bounds.size.width.into(),
                    bounds.size.height.into(),
                );
                assert!(left >= 0. && top >= 0. && width > 0. && height > 0.);
                assert!(
                    left + width <= 1441. && top + height <= 1001.,
                    "focused artwork is clipped by the window"
                );
                let x = (left * scale).round().max(0.) as u32;
                let y = (top * scale).round().max(0.) as u32;
                let w = ((width * scale).round() as u32).min(image.width().saturating_sub(x));
                let h = ((height * scale).round() as u32).min(image.height().saturating_sub(y));
                let mut colors = HashSet::new();
                for yy in (y..y + h).step_by((h / 96).max(1) as usize) {
                    for xx in (x..x + w).step_by((w / 128).max(1) as usize) {
                        colors.insert(image.get_pixel(xx, yy).0);
                    }
                }
                let file = format!("capture-{:04}.png", rows.len());
                image.save(output.join(&file)).unwrap();
                if row["expected"].is_null() {
                    return json!({"row":key,"status":"unresolved","decoded":false,"error":"The source fixture has no visual artwork for this row","capture":file,"pixel_bounds":[x,y,w,h],"window_pixels":[image.width(),image.height()]});
                }
                let expected = &row["expected"];
                assert!(
                    (actual["art_width"].as_f64().unwrap() - expected["width"].as_f64().unwrap())
                        .abs()
                        < 0.01
                );
                assert!(
                    (actual["art_height"].as_f64().unwrap() - expected["height"].as_f64().unwrap())
                        .abs()
                        < 0.01
                );
                assert_eq!(actual["hit_count"], expected["hit_count"]);
                let ratio =
                    expected["width"].as_f64().unwrap() / expected["height"].as_f64().unwrap();
                assert!(
                    ((width as f64 / height as f64) / ratio - 1.).abs() < 0.005,
                    "native layout changed the artwork aspect ratio"
                );
                assert!(
                    colors.len() > 8,
                    "the decoded artwork has not painted visible detail"
                );
                json!({"row":key,"status":"passed","decoded":true,"capture":file,
                    "dimensions":{"expected":[expected["width"],expected["height"]],"actual":[actual["art_width"],actual["art_height"]]},
                    "hit_count":{"expected":expected["hit_count"],"actual":actual["hit_count"]},
                    "element_bounds":[left,top,width,height],"pixel_bounds":[x,y,w,h],"window_pixels":[image.width(),image.height()],
                    "checks":[{"id":"production-piece-and-layout-input","passed":true},{"id":"native-aspect-and-visible-bounds","passed":true},{"id":"native-artwork-pixels","passed":true,"sampled_colors":colors.len()}]})
            }));
            rows.push(match result {
                Ok(row) => row,
                Err(error) => {
                    batch_failed = true;
                    let error = error
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| error.downcast_ref::<&str>().map(|s| s.to_string()))
                        .unwrap_or_else(|| "Native catalog input failed".into());
                    let mut failure = json!({"row":key,"status":"failed","error":error});
                    // Retain the actual failed view as well as its diagnostic state;
                    // a failed assertion must never leave only an unverifiable log.
                    if let Ok(image) =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            cx.capture_screenshot(handle)
                                .expect("capture failed native view")
                        }))
                    {
                        let file = format!("failure-{:04}.png", rows.len());
                        if image.save(output.join(&file)).is_ok() {
                            failure["capture"] = json!(file);
                            failure["pixel_bounds"] = json!([0, 0, image.width(), image.height()]);
                            failure["window_pixels"] = json!([image.width(), image.height()]);
                        }
                    }
                    failure
                }
            });
        }
    }
    let complete = rows.len() == inputs["scope"]["selected_rows"].as_u64().unwrap() as usize
        && rows.iter().all(|row| row["status"] == "passed");
    let manifest = json!({"schema_version":1,"kind":"native-catalog","provenance":{"candidate_sha":std::env::var("GITHUB_SHA").ok(),"fixture":inputs["fixture"],"environment":{"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"renderer":"WGPU"}},
        "scope":inputs["scope"],"method":"Real production focused view, piece/layout input, decoded images, geometry and hover-region counts; isolated fixtures do not call filesystem services", "rows":rows,"complete":complete});
    fs::write(
        output.join("capture-manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(
        complete,
        "Native catalog contains failed, empty or unexecuted desktop rows"
    );
}
