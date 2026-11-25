//! Compare native text/filters with the shared renderer's direct raster.
use creditia_gpui::{
    animation::{self, Event, PlaybackControl},
    native_preview, renderer,
    ui_model::{Artwork, Viewport, focused_rect, texture_width},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

fn shared_pixels(art: &Artwork, width: u32) -> Vec<u8> {
    let node =
        std::env::var_os("CREDITIA_NODE").expect("set CREDITIA_NODE for the reference renderer");
    let bridge = PathBuf::from(
        std::env::var_os("CREDITIA_RENDER_BRIDGE").expect("set CREDITIA_RENDER_BRIDGE"),
    );
    let module = bridge
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("src/native-protocol.ts");
    // Give both engines the same uniform transform and integer canvas. The
    // resvg-js width-fit option independently rounds its height, which changes
    // the sampling grid for every glyph in a tall image.
    let script = "import {readFileSync} from 'node:fs'; import {pathToFileURL} from 'node:url'; const {nativeBackend}=await import(pathToFileURL(process.argv[1]).href);const x=JSON.parse(readFileSync(0,'utf8'));const svg='<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"'+x.width+'\" height=\"'+Math.ceil(x.h*x.width/x.w)+'\"><g transform=\"scale('+x.width/x.w+')\">'+x.svg+'</g></svg>';process.stdout.write(nativeBackend.raster.png(svg));";
    let mut child = Command::new(node)
        .args(["--input-type=module", "-e", script])
        .arg(module)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            &serde_json::to_vec(&json!({"svg":art.svg,"width":width,"w":art.w,"h":art.h})).unwrap(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn production_frame(request: Value) -> (std::sync::Arc<Artwork>, u32, Vec<u8>) {
    let control = PlaybackControl::new();
    control.set_viewport(Viewport::default(), (1360., 777.), 2.);
    let mut frame = None;
    let result = animation::run(request, 2200, control.clone(), |event| {
        match event {
            Event::Started { times, .. } => control.set_playhead(times.len() / 2),
            Event::Frame {
                art, pixels, tiles, ..
            } if frame.is_none() => {
                let scale = f64::from(pixels) / f64::from(art.w);
                let height = (f64::from(art.h) * scale).ceil() as u32;
                let covered = tiles
                    .iter()
                    .map(|tile| u64::from(tile.width) * u64::from(tile.height))
                    .sum::<u64>();
                assert_eq!(
                    covered,
                    u64::from(pixels) * u64::from(height),
                    "this full-art comparison requires a fixture that fits the viewport; long fixtures must compare only their visible tile regions"
                );
                let mut bytes = vec![0; (u64::from(pixels) * u64::from(height) * 4) as usize];
                for tile in tiles {
                    let (x, y) = tile
                        .region
                        .map(|r| {
                            (
                                (f64::from(r.x) * scale).round() as u32,
                                (f64::from(r.y) * scale).round() as u32,
                            )
                        })
                        .unwrap_or_default();
                    let source = tile.image.as_bytes(0).unwrap();
                    assert!(x + tile.width <= pixels && y + tile.height <= height);
                    for row in 0..tile.height {
                        let from = (row * tile.width * 4) as usize;
                        let to = (((y + row) * pixels + x) * 4) as usize;
                        for (dst, src) in bytes[to..to + (tile.width * 4) as usize]
                            .as_chunks_mut::<4>()
                            .0
                            .iter_mut()
                            .zip(
                                source[from..from + (tile.width * 4) as usize]
                                    .as_chunks::<4>()
                                    .0,
                            )
                        {
                            dst.copy_from_slice(&[src[2], src[1], src[0], src[3]]);
                        }
                    }
                }
                let mut png = Vec::new();
                {
                    let mut encoder = png::Encoder::new(&mut png, pixels, height);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    encoder
                        .write_header()
                        .unwrap()
                        .write_image_data(&bytes)
                        .unwrap();
                }
                frame = Some((art, pixels, png));
                control.cancel();
            }
            _ => {}
        }
        Ok(())
    });
    assert!(control.user_cancelled(), "producer failed: {result:?}");
    frame.expect("production player never produced pixels")
}

fn compare(left: &resvg::tiny_skia::Pixmap, right: &resvg::tiny_skia::Pixmap) -> Value {
    assert_eq!(
        (left.width(), left.height()),
        (right.width(), right.height())
    );
    let differences = left
        .data()
        .iter()
        .zip(right.data())
        .map(|(a, b)| a.abs_diff(*b))
        .collect::<Vec<_>>();
    let mse = differences
        .iter()
        .map(|v| f64::from(*v).powi(2))
        .sum::<f64>()
        / differences.len() as f64;
    let psnr = if mse == 0. {
        None
    } else {
        Some(10. * (255f64.powi(2) / mse).log10())
    };
    // Cached offscreen composites quantize premultiplied color separately.
    // Report their measured difference; do not relabel it pixel equivalence
    // or silently apply the font comparison's acceptance threshold here.
    json!({"pixel_identical":mse==0.,"psnr_db":psnr,"mean_channel_difference":differences.iter().map(|v| f64::from(*v)).sum::<f64>()/differences.len() as f64,"fraction_pixels_over_8":differences.as_chunks::<4>().0.iter().filter(|pixel| pixel.iter().any(|v| *v>8)).count() as f64/(differences.len()/4) as f64,"max_difference":differences.iter().max()})
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fixture: Value = serde_json::from_slice(&fs::read(&args[1]).unwrap()).unwrap();
    let raw = fixture.get("raw").unwrap_or(&fixture);
    let output = PathBuf::from(&args[2]);
    fs::create_dir_all(&output).unwrap();
    let mut cases = Vec::new();
    let catalog = renderer::request(
        json!({"version":1,"id":"conformance-catalog","op":"catalog"}),
        renderer::Cancellation::default(),
    )
    .unwrap();
    let mut selections = catalog["styles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|style| {
            (
                style["id"].as_str().unwrap().to_owned(),
                "tier",
                None,
                false,
            )
        })
        .collect::<Vec<_>>();
    for style in ["minimal-light", "cinema"] {
        selections.push((style.into(), "roll", Some("scroll"), false));
    }
    for style in ["baroque", "grimoire"] {
        selections.push((style.into(), "tier", None, true));
    }
    for (style, piece, variant, fallback) in selections {
        for lang in ["en", "zh"] {
            let mut images = Vec::new();
            let mut archive = raw.clone();
            if fallback {
                archive["project"]["name"] = json!("Ω → 中文");
            }
            {
                let mut request = json!({"version":1,"id":"conformance","op":"render","raw":archive,"style":style,"piece":piece,"lang":lang,"group":"model","named":true,"t":0.5,"flatten":false});
                if let Some(variant) = variant {
                    request["variant"] = json!(variant);
                }
                let result = renderer::request(request, renderer::Cancellation::default()).unwrap();
                let art = Artwork::from_result(&result).unwrap();
                let rect = focused_rect(art.w, art.h, Viewport::default(), (1360., 777.));
                let width = texture_width(rect.w, 2.);
                let raster = native_preview::rasterize_artwork(
                    &art,
                    width,
                    &renderer::Cancellation::default(),
                )
                .unwrap();
                for (mode, png) in [
                    ("native", raster.png.clone()),
                    ("shared", shared_pixels(&art, width)),
                ] {
                    fs::write(
                        output.join(format!(
                            "{style}-{piece}-{lang}-{}-{}.png",
                            if fallback { "fallback" } else { "standard" },
                            mode
                        )),
                        &png,
                    )
                    .unwrap();
                    images.push(resvg::tiny_skia::Pixmap::decode_png(&png).unwrap());
                }
            }
            assert_eq!(
                (images[0].width(), images[0].height()),
                (images[1].width(), images[1].height())
            );
            let differences = images[0]
                .data()
                .iter()
                .zip(images[1].data())
                .map(|(a, b)| a.abs_diff(*b))
                .collect::<Vec<_>>();
            let mean =
                differences.iter().map(|v| f64::from(*v)).sum::<f64>() / differences.len() as f64;
            let fraction = differences
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|pixel| pixel.iter().any(|v| *v > 8))
                .count() as f64
                / (differences.len() / 4) as f64;
            let mse = differences
                .iter()
                .map(|v| f64::from(*v).powi(2))
                .sum::<f64>()
                / differences.len() as f64;
            let psnr = if mse == 0. {
                None
            } else {
                Some(10. * (255f64.powi(2) / mse).log10())
            };
            let row = json!({"style":style,"piece":piece,"variant":variant,"lang":lang,"fallback":fallback,"pixel_identical":mse==0. && images[0].height()==images[1].height(),"common_rows_identical":mse==0.,"native_height":images[0].height(),"shared_height":images[1].height(),"psnr_db":psnr,"mean_channel_difference":mean,"fraction_pixels_over_8":fraction,"max_difference":differences.iter().max()});
            println!("{row}");
            // Different resvg releases also differ in edge/filter sampling.
            // Preserve the measurements for visual review; emitting this row
            // does not certify pixel identity or replace the shared SVG gate.
            cases.push(row);
            if !fallback
                && ["minimal-light", "cinema", "cyber", "ink", "glass"].contains(&style.as_str())
            {
                let mut request = json!({"version":1,"id":"production-conformance","raw":archive,"style":style,"piece":piece,"lang":lang,"group":"model","named":true});
                if let Some(variant) = variant {
                    request["variant"] = json!(variant);
                }
                let (art, width, png) = production_frame(request);
                fs::write(
                    output.join(format!("{style}-{piece}-{lang}-production.png")),
                    &png,
                )
                .unwrap();
                resvg::set_turbulence_cache_budget(0);
                let original = native_preview::rasterize_artwork(
                    &art,
                    width,
                    &renderer::Cancellation::default(),
                )
                .unwrap();
                fs::write(
                    output.join(format!("{style}-{piece}-{lang}-full-svg.png")),
                    &original.png,
                )
                .unwrap();
                let diff = compare(
                    &resvg::tiny_skia::Pixmap::decode_png(&png).unwrap(),
                    &resvg::tiny_skia::Pixmap::decode_png(&original.png).unwrap(),
                );
                let row = json!({"style":style,"piece":piece,"lang":lang,"comparison":"production-dual-lane-layer-noise-caches-vs-full-svg","difference":diff});
                println!("{row}");
                cases.push(row);
            }
        }
    }
    fs::write(output.join("manifest.json"),serde_json::to_vec_pretty(&json!({"method":"identical source, phase, fonts and physical width; native text vs shared direct raster, plus separately recorded production cache comparisons","cases":cases})).unwrap()).unwrap();
}
