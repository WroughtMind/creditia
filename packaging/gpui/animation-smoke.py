#!/usr/bin/env python3
"""Exercise the installed streaming player through real X11 input at native DPI2.

No pre-rendered fixtures, test-only mutations, renderer overrides or synthetic
window are used. Read-only app diagnostics identify the images which reached the
normal decoder; OS captures independently retain their painted pixels.
"""
import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

from PIL import Image, ImageChops


MIB = 1024 * 1024


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def frame_ready(state):
    player = state.get("animation") or {}
    return bool(state.get("viewport_ready") and player.get("decode_ready")
                and player.get("epoch") == player.get("displayed_epoch")
                and player.get("chunks"))


def image_assets(state):
    assets = state.get("image_assets") or {}
    for key in ("tracked_sources", "decoded_images", "decoded_bytes", "atlas_releases"):
        if type(assets.get(key)) is not int or assets[key] < 0:
            raise AssertionError(f"Missing native image retirement diagnostics: {assets}")
    if not (0 < assets["decoded_images"] <= assets["tracked_sources"] and assets["decoded_bytes"] > 0):
        raise AssertionError(f"Ready wall contains no tracked decoded artwork: {assets}")
    return assets


def validate_frame(state):
    if state.get("display_scale") != 2:
        raise AssertionError(f"Expected native scale2, observed {state.get('display_scale')}")
    if not frame_ready(state):
        raise AssertionError("Displayed frame has not decoded at the current viewport epoch")
    player, rect = state["animation"], state["focused_rect"]
    # Focus geometry is f32. Ignore representation error at an integer pixel,
    # using the same tolerance as the production texture_width calculation.
    physical = rect["w"] * state["display_scale"]
    nearest = round(physical)
    tolerance = max(0.0001, abs(physical) * 2 ** -22)
    required = nearest if abs(physical - nearest) <= tolerance else math.ceil(physical)
    if player["frame_pixels"] < required:
        raise AssertionError(f"Undersampled frame: {player['frame_pixels']} < {required}")
    decoded = 0
    for chunk in player["chunks"]:
        width, height = chunk["width"], chunk["height"]
        if not (isinstance(width, int) and isinstance(height, int) and width > 0 and height > 0):
            raise AssertionError(f"Invalid decoded frame chunk: {chunk}")
        cost = width * height * 4
        if cost > 8 * MIB:
            raise AssertionError(f"Frame chunk exceeds8MiB: {cost}")
        decoded += cost
    if not (decoded <= player["resident_bytes"] <= 64 * MIB):
        raise AssertionError(f"Invalid resident frame budget: {decoded}, {player['resident_bytes']}")
    if not 1 <= player["resident_frames"] <= 8:
        raise AssertionError(f"Invalid resident frame count: {player['resident_frames']}")
    if state.get("art_width") and state.get("art_height") and state.get("viewport"):
        validate_coverage(state)
    return {"equivalent_pixels": player["frame_pixels"], "required_pixels": required,
            "chunks": len(player["chunks"]), "current_decoded_bytes": decoded,
            "resident_bytes": player["resident_bytes"], "resident_frames": player["resident_frames"]}


def validate_coverage(state):
    """Require source-coordinate tiles to cover the visible artwork, not just
    report a high equivalent width for a tiny, unrelated cropped bitmap."""
    player, rect = state["animation"], state["focused_rect"]
    aw, ah = state["art_width"], state["art_height"]
    density = player["frame_pixels"] / aw
    logical_scale = density / state["display_scale"]
    viewport = state["viewport"]
    target = (max(0., -rect["x"] / logical_scale), max(0., -rect["y"] / logical_scale),
              min(aw, (viewport["width"] - rect["x"]) / logical_scale),
              min(ah, (viewport["height"] - 154 - rect["y"]) / logical_scale))
    x0, y0, x1, y1 = target
    if x1 <= x0 or y1 <= y0:
        raise AssertionError("Frame marked visible has no visible artwork area")
    regions = [chunk.get("region") or {"x": 0., "y": 0., "w": aw, "h": ah}
               for chunk in player["chunks"]]
    # One physical pixel accommodates source-coordinate f32 roundoff and the
    # final edge ceil; a missing tile or pan window leaves a much larger hole.
    epsilon = 1. / density
    edges = sorted({x0, x1, *(max(x0, min(x1, value)) for region in regions
                             for value in (region["x"], region["x"] + region["w"]))})
    for left, right in zip(edges, edges[1:]):
        if right - left <= epsilon:
            continue
        middle, covered = (left + right) / 2, y0
        intervals = sorted((region["y"], region["y"] + region["h"]) for region in regions
                           if region["x"] - epsilon <= middle <= region["x"] + region["w"] + epsilon)
        for start, end in intervals:
            if start > covered + epsilon:
                break
            covered = max(covered, end)
        if covered < y1 - epsilon:
            raise AssertionError(f"Frame chunks leave a visible gap: x={left}:{right}, y={covered}:{y1}")


class Events:
    def __init__(self, path):
        self.path, self.offset, self.pending = path, 0, b""
        self.state, self.sequence = {}, 0
        self.frame_events = []
        self.last_frame_state = None

    def read(self):
        if self.path.exists():
            with self.path.open("rb") as stream:
                stream.seek(self.offset)
                self.pending += stream.read()
                self.offset = stream.tell()
            lines = self.pending.split(b"\n")
            self.pending = lines.pop()
            for line in lines:
                if not line:
                    continue
                event = json.loads(line)
                if isinstance(event.get("state"), dict):
                    self.state = event["state"]
                    self.sequence += 1
                    player = self.state.get("animation")
                    if player:
                        identity = tuple(player.get(k) for k in ("epoch", "displayed_epoch", "index", "playing", "suspended", "decode_ready"))
                        if identity != self.last_frame_state:
                            self.frame_events.append({"time_ms": event.get("timeMs"),
                                                      "observed_ms": time.time_ns() / 1_000_000,
                                                      "animation": player})
                            self.last_frame_state = identity
        return self.state


def cadence(events, start_ms, end_ms):
    rows = [row for row in events if row.get("time_ms") is not None and start_ms <= row["time_ms"] <= end_ms]
    intervals, skips, previous, displayed = [], 0, None, 0
    for row in rows:
        frame = row["animation"]
        if not (frame.get("playing") and not frame.get("suspended") and frame.get("decode_ready")
                and frame.get("epoch") == frame.get("displayed_epoch")):
            previous = None
            continue
        if previous:
            old = previous["animation"]
            if old["epoch"] == frame["epoch"] and frame["index"] > old["index"]:
                intervals.append({"ms": row["time_ms"] - previous["time_ms"], "from": old["index"], "to": frame["index"]})
                skips += max(0, frame["index"] - old["index"] - 1)
            elif old["epoch"] == frame["epoch"] and frame["index"] == old["index"]:
                continue
        displayed += 1
        previous = row
    values = sorted(row["ms"] for row in intervals)
    percentile = lambda q: values[min(len(values) - 1, max(0, math.ceil(q * len(values)) - 1))] if values else None
    return {"measurement": "decoded frame-selection timestamps before GPU presentation",
            "displayed_samples": displayed, "interval_count": len(values), "skipped_sample_indices": skips,
            "interval_ms": {"p50": percentile(.5), "p95": percentile(.95), "max": max(values) if values else None},
            "gaps_over_250ms": [row for row in intervals if row["ms"] > 250], "intervals": intervals,
            "limits": "No compositor presentation fence timestamps; independent OS screenshots verify sampled painted output."}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--archive", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--inside-x11", action="store_true")
    args = parser.parse_args()
    if sys.platform != "linux":
        raise RuntimeError("Native DPI2 streaming validation currently requires Linux/X11")
    if not args.inside_x11:
        return subprocess.run(["xvfb-run", "-a", "-s", "-screen 0 3200x2000x24",
                               sys.executable, str(Path(__file__).resolve()), *sys.argv[1:],
                               "--inside-x11"], timeout=420).returncode
    binary, archive, output = (Path(p).resolve() for p in (args.binary, args.archive, args.output))
    output.mkdir(parents=True, exist_ok=True)
    report = {"schema_version": 1, "kind": "installed-streaming-animation-dpi2", "complete": False,
              "candidate_sha": args.candidate, "binary_sha256": sha(binary), "archive_sha256": sha(archive),
              "driver_sha256": sha(Path(__file__)), "checks": [], "actions": [],
              "limits": ["Linux/X11 software renderer; does not establish macOS/Windows GPU behavior.",
                         "Process RSS sums resident sets and excludes GPU/driver memory.",
                         "Atlas releases count completed calls to GPUI's sprite-atlas removal. Decoded bytes count unique tracked CPU images, not measured GPU allocations."]}
    resources_path = Path(__file__).resolve().parent.parent / "gpui-acceptance/resources.py"
    spec = importlib.util.spec_from_file_location("creditia_resources", resources_path)
    resources = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(resources)
    process = wm = None
    handles, retained, samples = [], {}, []
    with tempfile.TemporaryDirectory(prefix="creditia-native-animation-") as temporary:
        work = Path(temporary)
        fixture = work / "archive.json"
        fixture.write_bytes(archive.read_bytes())
        runtime = work / "runtime"
        runtime.mkdir(mode=0o700)
        events_file = output / "application-events.jsonl"
        events_file.unlink(missing_ok=True)
        observed = Events(events_file)
        env = dict(os.environ)
        for key in ("CREDITIA_CORE", "CREDITIA_NODE", "CREDITIA_RENDER_BRIDGE", "NODE_PATH", "NODE_OPTIONS",
                    "CREDITIA_PERF_EVENTS", "WAYLAND_DISPLAY", "GDK_SCALE", "GDK_DPI_SCALE"):
            env.pop(key, None)
        env.update(CREDITIA_HOME=str(work / "library"), CREDITIA_APP_EVENTS=str(events_file),
                   XDG_RUNTIME_DIR=str(runtime), XDG_SESSION_TYPE="x11", GPUI_X11_SCALE_FACTOR="2")

        def run(command, timeout=15, check=True):
            return subprocess.run([str(x) for x in command], env=env, cwd=work, check=check,
                                  capture_output=True, text=True, timeout=timeout)

        def xdo(*command):
            return run(["xdotool", *command]).stdout.strip()

        def sample(label):
            table = resources.processes()
            pids = resources.descendants(process.pid, table, retained)
            for pid in pids:
                retained[pid] = table[pid]["identity"]
            rows = [table[pid] for pid in sorted(pids)]
            value = {"phase": label, "monotonic_seconds": time.monotonic(), "processes": rows,
                     "rss_bytes": sum(row["rss_bytes"] for row in rows),
                     "cpu_seconds": sum(row["cpu_seconds"] for row in rows)}
            samples.append(value)
            return value

        def wait(predicate, label, timeout=45, after=None):
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError(f"Application exited {process.returncode} during {label}")
                state = observed.read()
                if (after is None or observed.sequence > after) and predicate(state):
                    return state
                time.sleep(.03)
            raise TimeoutError(f"No observed {label}: {observed.state}")

        def action(label, operation, predicate):
            before = observed.read()
            sequence, started = observed.sequence, time.monotonic()
            operation()
            state = wait(lambda state: predicate(before, state), label, after=sequence)
            report["actions"].append({"action": label, "before": before, "after": state,
                                      "elapsed_ms": (time.monotonic() - started) * 1000})
            sample(label)
            return state

        def key(value):
            xdo("key", "--clearmodifiers", value)

        def capture(name, state):
            file = output / f"{name}.png"
            run(["import", "-window", window, str(file)])
            with Image.open(file) as image:
                if image.width < 2000 or image.height < 1000:
                    raise AssertionError(f"DPI2 screenshot has unexpected physical size {image.size}")
                dimensions = image.size
            report["checks"].append({"id": name, "passed": True, "state": state,
                                     "capture": file.name, "physical_size": dimensions,
                                     "frame": validate_frame(state) if state.get("animation") else None})
            return file

        try:
            wm_log = (output / "window-manager.log").open("wb")
            handles.append(wm_log)
            wm = subprocess.Popen(["openbox"], cwd=work, env=env, stdout=wm_log, stderr=wm_log)
            deadline = time.monotonic() + 10
            while "window id" not in run(["xprop", "-root", "_NET_SUPPORTING_WM_CHECK"], check=False).stdout:
                if wm.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError("X11 window manager did not become ready")
                time.sleep(.1)
            for name in ("stdout.log", "stderr.log"):
                handles.append((output / name).open("wb"))
            process = subprocess.Popen([str(binary), "--benchmark-archive", str(fixture)], cwd=work,
                                       env=env, stdout=handles[-2], stderr=handles[-1], start_new_session=True)
            window, deadline = None, time.monotonic() + 120
            while time.monotonic() < deadline:
                found = run(["xdotool", "search", "--onlyvisible", "--pid", process.pid], check=False)
                if found.returncode == 0 and found.stdout.strip():
                    window = found.stdout.strip().splitlines()[-1]
                    break
                if process.poll() is not None:
                    raise RuntimeError(f"Application exited {process.returncode} before creating a window")
                time.sleep(.1)
            if window is None:
                raise TimeoutError("No installed native window")
            xdo("windowactivate", "--sync", window)
            state = wait(lambda s: s.get("viewport_ready") and s.get("first_tile"), "initial viewport", 150)
            if state.get("display_scale") != 2:
                raise AssertionError(f"Native scale is not2: {state}")
            if state.get("style") != "minimal-light" or state.get("selected_piece") != "share":
                raise AssertionError(f"Unexpected animation acceptance scope: {state}")
            initial_assets = image_assets(state)
            capture("wall-dpi2", state)
            action("focus", lambda: key("Return"), lambda _, s: s.get("focused") and s.get("viewport_ready"))
            # Space uses the normal Play action and starts the live renderer.
            action("play", lambda: key("space"), lambda _, s: s.get("playing") and frame_ready(s))
            wait(lambda s: frame_ready(s) and s["animation"]["index"] >= 2, "progressive frame")
            state = action("pause", lambda: key("space"), lambda _, s: not s.get("playing") and frame_ready(s))
            first_index = state["animation"]["index"]
            first = capture("paused-first", state)
            action("resume", lambda: key("space"), lambda _, s: s.get("playing") and frame_ready(s))
            wait(lambda s: frame_ready(s) and abs(s["animation"]["index"] - first_index) > 2,
                 "different painted frame after resume")
            state = action("pause-later", lambda: key("space"), lambda _, s: not s.get("playing") and frame_ready(s))
            second = capture("paused-later", state)
            with Image.open(first) as a, Image.open(second) as b:
                # Exclude all shell controls and the stationary mouse: the
                # comparison must demonstrate changing artwork pixels.
                rect, dpi = state["focused_rect"], state["display_scale"]
                crop = (max(0, math.ceil(rect["x"] * dpi)), max(0, math.ceil((rect["y"] + 60) * dpi)),
                        min(a.width, math.floor((rect["x"] + rect["w"]) * dpi)),
                        min(a.height - 154 * dpi, math.floor((rect["y"] + 60 + rect["h"]) * dpi)))
                difference = ImageChops.difference(a.convert("RGB").crop(crop), b.convert("RGB").crop(crop))
                bounds = difference.getbbox()
                if bounds is None:
                    raise AssertionError("Different live frame indices produced identical window pixels")
                report["checks"].append({"id": "progressive-painted-frame-change", "passed": True,
                                         "changed_bounds": bounds, "from_index": first_index,
                                         "to_index": state["animation"]["index"]})
            paused_index = state["animation"]["index"]

            def zoom():
                for _ in range(5):
                    key("ctrl+equal")

            state = action("paused-deep-zoom", zoom,
                           lambda old, s: s.get("zoom", 0) > old.get("zoom", 1) * 2
                           and frame_ready(s) and s["animation"]["epoch"] > old["animation"]["epoch"])
            if state.get("playing") or state["animation"]["index"] != paused_index:
                raise AssertionError("Paused zoom advanced the animation clock")
            if not any(chunk.get("region") is not None for chunk in state["animation"]["chunks"]):
                raise AssertionError("Deep zoom did not use bounded visible-region chunks")
            capture("paused-deep-zoom", state)

            def pan():
                geometry = dict(line.split("=", 1) for line in xdo("getwindowgeometry", "--shell", window).splitlines())
                width, height = int(geometry["WIDTH"]), int(geometry["HEIGHT"])
                xdo("mousemove", "--window", window, width // 2, height // 2)
                xdo("mousedown", 1)
                xdo("mousemove", "--window", window, width // 2 + 190, height // 2 + 110)
                xdo("mouseup", 1)

            state = action("paused-pan", pan,
                           lambda old, s: (s.get("pan_x"), s.get("pan_y")) != (old.get("pan_x"), old.get("pan_y"))
                           and frame_ready(s) and s["animation"]["epoch"] > old["animation"]["epoch"])
            if state.get("playing") or state["animation"]["index"] != paused_index:
                raise AssertionError("Paused pan advanced the animation clock")
            capture("paused-pan", state)
            # A user pause must survive invisibility; restoration refreshes only
            # the same frame, then explicit replay resets the canonical timeline.
            action("hide-paused", lambda: xdo("windowminimize", window), lambda _, s: not s.get("visible", True))
            time.sleep(.3)
            before_idle = sample("hidden-start")
            hidden = observed.read()
            for _ in range(10):
                time.sleep(.2)
                sample("hidden")
            after_idle = sample("hidden-end")
            if observed.read().get("animation", {}).get("index") != hidden["animation"]["index"]:
                raise AssertionError("Invisible paused animation advanced")
            report["checks"].append({"id": "hidden-paused-resource-sample", "passed": True,
                                     "elapsed_seconds": after_idle["monotonic_seconds"] - before_idle["monotonic_seconds"],
                                     "cpu_seconds_delta": after_idle["cpu_seconds"] - before_idle["cpu_seconds"],
                                     "rss_start": before_idle["rss_bytes"], "rss_end": after_idle["rss_bytes"]})
            state = action("restore-paused", lambda: xdo("windowactivate", "--sync", window),
                           lambda _, s: s.get("visible") and s.get("active") and frame_ready(s))
            if state.get("playing") or state["animation"]["index"] != paused_index:
                raise AssertionError("Restoring a user-paused animation resumed its clock")
            capture("restored-paused", state)
            state = action("replay", lambda: key("ctrl+shift+space"),
                           lambda old, s: s.get("playing") and frame_ready(s)
                           and s["animation"]["epoch"] > old["animation"]["epoch"]
                           and s["animation"].get("elapsed_ms") == 0)
            validate_frame(state)
            action("hide-playing", lambda: xdo("windowminimize", window),
                   lambda _, s: not s.get("visible", True) and (s.get("animation") or {}).get("suspended"))
            hidden = observed.read()
            if not hidden["animation"].get("intent_playing"):
                raise AssertionError("Invisibility discarded the user's playback intent")
            time.sleep(.3)  # Permit an already-running frame to finish.
            before_idle = sample("hidden-playing-start")
            for _ in range(10):
                time.sleep(.2)
                sample("hidden-playing")
            after_idle = sample("hidden-playing-end")
            after_hidden = observed.read()
            if tuple(after_hidden["animation"][key] for key in ("index", "epoch", "elapsed_ms")) != tuple(
                    hidden["animation"][key] for key in ("index", "epoch", "elapsed_ms")):
                raise AssertionError("Invisible playing animation advanced its suspended clock")
            report["checks"].append({"id": "hidden-playing-resource-sample", "passed": True,
                                     "elapsed_seconds": after_idle["monotonic_seconds"] - before_idle["monotonic_seconds"],
                                     "cpu_seconds_delta": after_idle["cpu_seconds"] - before_idle["cpu_seconds"],
                                     "rss_start": before_idle["rss_bytes"], "rss_end": after_idle["rss_bytes"]})
            state = action("restore-playing", lambda: xdo("windowactivate", "--sync", window),
                           lambda _, s: s.get("visible") and s.get("playing") and frame_ready(s)
                           and not s["animation"].get("suspended"))
            validate_frame(state)
            if state.get("playing"):
                action("pause-before-cadence", lambda: key("space"), lambda _, s: not s.get("playing") and frame_ready(s))
            state = action("reset-for-cadence", lambda: key("ctrl+0"),
                           lambda _, s: s.get("zoom") == 1 and frame_ready(s))
            cadence_start = time.time_ns() / 1_000_000
            state = action("uninterrupted-replay", lambda: key("ctrl+shift+space"),
                           lambda old, s: s.get("playing") and frame_ready(s)
                           and s["animation"]["epoch"] > old["animation"]["epoch"])
            timeline = state["animation"].get("timeline") or {}
            duration_ms = timeline.get("duration_ms", 2000)
            until = time.monotonic() + min(12, max(2, duration_ms / 1000 + .5))
            while time.monotonic() < until:
                if process.poll() is not None:
                    raise RuntimeError("Application exited during uninterrupted replay")
                observed.read()
                sample("uninterrupted-replay")
                time.sleep(.05)
            cadence_end = time.time_ns() / 1_000_000
            report["cadence"] = cadence(observed.frame_events, cadence_start, cadence_end)
            report["cadence"]["timeline"] = timeline
            report["cadence"]["replay_to_current_decoded_frame_ms"] = report["actions"][-1]["elapsed_ms"]
            if report["cadence"]["interval_count"] < 2:
                raise AssertionError("Uninterrupted replay supplied fewer than3 decoded frame selections")
            state = observed.read()
            if state.get("playing"):
                state = action("pause-after-cadence", lambda: key("space"), lambda _, s: not s.get("playing") and frame_ready(s))
            capture("replay-complete", state)
            state = action("close-player", lambda: key("Escape"), lambda _, s: not s.get("focused") and not s.get("animation") and s.get("viewport_ready"))
            final_assets = image_assets(state)
            if final_assets["atlas_releases"] <= initial_assets["atlas_releases"]:
                raise AssertionError("Streaming, viewport replacement and playback teardown did not retire any sprite-atlas entries")
            report["checks"].append({"id": "decoder-and-atlas-retirement", "passed": True,
                                     "wall_before": initial_assets, "wall_after": final_assets,
                                     "completed_release_calls": final_assets["atlas_releases"] - initial_assets["atlas_releases"]})
            key("ctrl+q")
            code = process.wait(timeout=20)
            if code != 0:
                raise RuntimeError(f"Installed app did not quit normally: {code}")
            report["checks"].append({"id": "normal-quit", "passed": True, "exit_code": code})
            report["complete"] = True
        except Exception as error:
            report["error"] = f"{type(error).__name__}: {error}"
            report["last_state"] = observed.read()
            if process is not None and process.poll() is None and window is not None:
                run(["import", "-window", window, str(output / "failure.png")], check=False)
        finally:
            if process is not None:
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                if process.poll() is None:
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait()
            if wm is not None:
                wm.terminate()
                try:
                    wm.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    wm.kill()
                    wm.wait()
            for handle in handles:
                handle.close()
            (output / "process-samples.jsonl").write_text("".join(json.dumps(row) + "\n" for row in samples))
            (output / "frame-selections.jsonl").write_text("".join(json.dumps(row) + "\n" for row in observed.frame_events))
            report["artifacts"] = [{"path": file.name, "bytes": file.stat().st_size, "sha256": sha(file)}
                                   for file in sorted(output.iterdir())
                                   if file.is_file() and file.name != "animation-evidence.json"]
            (output / "animation-evidence.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"complete": report["complete"], "error": report.get("error")}))
    return 0 if report["complete"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
