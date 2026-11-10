//! Measure the real producer at native display density, without desktop input.
//! This reports prepared pixels, not compositor presentation or monitor FPS.
use creditia_gpui::{
    animation::{self, Event, PlaybackControl},
    ui_model::{Timeline, Viewport, frame_at},
};
use serde_json::{Value, json};
use std::{
    fs,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fixture: Value =
        serde_json::from_slice(&fs::read(&args[1]).expect("archive/fixture path")).unwrap();
    let raw = fixture.get("raw").unwrap_or(&fixture).clone();
    let style = args.get(2).map(String::as_str).unwrap_or("minimal-light");
    let piece = args.get(3).map(String::as_str).unwrap_or("roll");
    let variant = args.get(4).filter(|value| value.as_str() != "-");
    let seconds: f64 = args
        .get(5)
        .map(|value| value.parse().expect("profile seconds"))
        .unwrap_or(5.);
    let control = PlaybackControl::new();
    control.set_viewport(Viewport::default(), (1360., 777.), 2.);
    let producer = control.clone();
    let (tx, rx) = mpsc::sync_channel(3);
    let mut request = json!({"version":1,"id":"playback-profile","raw":raw,"style":style,"piece":piece,"lang":"en","group":"model","named":true});
    if let Some(variant) = variant {
        request["variant"] = json!(variant);
    }
    let worker = thread::spawn(move || {
        animation::run(request, 2200, producer, |event| {
            tx.send(event).map_err(|_| "profile finished".into())
        })
    });
    let mut timeline = Timeline::default();
    let mut times = vec![];
    let mut started = None;
    let mut samples = vec![];
    let mut max_bytes = 0;
    let mut max_tile_bytes = 0;
    loop {
        if let Some(start) = started {
            let elapsed = Instant::now().duration_since(start);
            if elapsed.as_secs_f64() >= seconds {
                break;
            }
            control.set_playhead(frame_at(&times, &timeline, elapsed.as_millis() as u64));
        }
        match rx.recv_timeout(Duration::from_millis(2)) {
            Ok(Event::Started {
                timeline: metadata,
                times: sampling,
                ..
            }) => {
                timeline = metadata;
                times = sampling;
            }
            Ok(Event::Frame {
                index,
                tiles,
                decoded_bytes,
                generation_micros,
                raster_micros,
                ..
            }) => {
                let start = *started.get_or_insert_with(Instant::now);
                samples.push(json!({"ms":start.elapsed().as_secs_f64()*1000.,"index":index,"generation_us":generation_micros,"raster_us":raster_micros}));
                max_bytes = max_bytes.max(decoded_bytes);
                max_tile_bytes = max_tile_bytes.max(
                    tiles
                        .iter()
                        .map(|tile| tile.image.as_bytes(0).unwrap().len())
                        .max()
                        .unwrap_or(0),
                );
            }
            Err(mpsc::RecvTimeoutError::Timeout) => assert!(
                !worker.is_finished(),
                "producer failed before profile completed"
            ),
            Err(error) => panic!("producer stopped: {error}; {:?}", worker.join().unwrap()),
        }
    }
    control.cancel();
    drop(rx);
    let _ = worker.join().unwrap();
    let mut gaps: Vec<f64> = samples
        .windows(2)
        .map(|pair| pair[1]["ms"].as_f64().unwrap() - pair[0]["ms"].as_f64().unwrap())
        .collect();
    gaps.sort_by(f64::total_cmp);
    let percentile = |p: f64| gaps[(gaps.len().saturating_sub(1) as f64 * p).round() as usize];
    println!("{}", serde_json::to_string_pretty(&json!({"method":"real SVG generation + native raster at DPI 2; prepared frames, not display FPS","style":style,"piece":piece,"variant":variant,"timeline_ms":timeline.duration_ms,"samples":times.len(),"prepared_per_second":samples.len() as f64/seconds,"gap_p50_ms":percentile(0.5),"gap_p95_ms":percentile(0.95),"largest_frame_bytes":max_bytes,"largest_tile_bytes":max_tile_bytes,"frames":samples})).unwrap());
}
