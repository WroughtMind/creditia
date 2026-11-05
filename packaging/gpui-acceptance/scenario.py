#!/usr/bin/env python3
"""Drive real Linux app windows through X11 input and verify observed UI state.

Run under resources.py. Both applications must be instrumented to emit passive
CREDITIA_APP_EVENTS telemetry. This driver never calls production view functions
or injects synthetic DOM input. Its own process overhead is included in both
whole-tree measurements. Event hashes prove artifact integrity, not authenticity
against a malicious app or CI host.
"""
import argparse
import hashlib
import json
import os
import pathlib
import signal
import subprocess
import sys
import time


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


class Journal:
    def __init__(self, path):
        self.path, self.previous, self.sequence = path, "0" * 64, 0
        self.started = time.monotonic()

    def write(self, event, **fields):
        self.sequence += 1
        value = {"event": event, "sequence": self.sequence, "elapsed_seconds": time.monotonic() - self.started,
                 "previous_sha256": self.previous, "monotonic_seconds": time.monotonic(), **fields}
        digest = hashlib.sha256(canonical(value).encode()).hexdigest()
        with self.path.open("a") as output:
            output.write(canonical({**value, "sha256": digest}) + "\n")
        self.previous = digest


def verify_journal(events):
    previous = "0" * 64
    for sequence, event in enumerate(events, 1):
        value = dict(event)
        digest = value.pop("sha256", None)
        if value.get("sequence") != sequence or value.get("previous_sha256") != previous:
            return False
        if digest != hashlib.sha256(canonical(value).encode()).hexdigest():
            return False
        previous = digest
    return bool(events)


def run(args):
    if not sys.platform.startswith("linux") or not os.environ.get("DISPLAY"):
        raise RuntimeError("This scenario driver requires a real Linux X11 display (Xvfb is supported)")
    command = args.command[1:] if args.command and args.command[0] == "--" else args.command
    if not command:
        raise ValueError("Application command required after --")
    fixture = pathlib.Path(args.fixture).resolve()
    event_path = pathlib.Path(os.environ["CREDITIA_PERF_EVENTS"]).resolve()
    event_path.parent.mkdir(parents=True, exist_ok=True)
    if event_path.exists():
        raise ValueError("Use a fresh evidence output directory for each run")
    app_events = event_path.with_name("application-events.jsonl")
    journal = Journal(event_path)
    started = time.monotonic()
    env = dict(os.environ, CREDITIA_APP_EVENTS=str(app_events), CREDITIA_ACCEPTANCE_ARCHIVE=str(fixture), CREDITIA_ACCEPTANCE_MODE="1")
    process = subprocess.Popen(command, env=env)
    journal.write("launch", application=args.application, scenario=args.scenario, cache_state=args.cache,
                  fixture_sha256=hashlib.sha256(fixture.read_bytes()).hexdigest(),
                  driver_sha256=hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
                  workload="focus-enlargement" if args.scenario == "zoom" else args.scenario)
    window = None

    def xdo(*command):
        return subprocess.run(["xdotool", *map(str, command)], check=True, capture_output=True, text=True).stdout.strip()

    def observed():
        values = []
        if app_events.exists():
            for line in app_events.read_text().splitlines():
                try:
                    event = json.loads(line)
                    if isinstance(event.get("state"), dict):
                        values.append(event)
                except (json.JSONDecodeError, AttributeError):
                    pass  # Last append may not have completed yet.
        return values

    def wait_state(predicate, description, timeout=30):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError(f"Application exited with {process.returncode} while waiting for {description}")
            events = observed()
            if events and predicate(events[-1]["state"]):
                return events[-1]["state"]
            time.sleep(.05)
        raise TimeoutError(f"No observed UI state for {description}; latest={observed()[-1:]}")

    def snapshot(label):
        time.sleep(.1)  # Allow the verified render to reach the X11 compositor.
        file = event_path.with_name(f"{label}.png")
        subprocess.run(["import", "-window", str(window), str(file)], capture_output=True, check=True)
        # DOM/image readiness can precede (or never reach) an X11 paint, notably
        # with WebKitGTK software compositing. Reject the blank initial shell.
        pixels = subprocess.run(["convert", str(file), "-crop", "1200x680+80+100", "+repage", "-depth", "8", "rgb:-"], capture_output=True, check=True).stdout
        dark = sum(max(pixels[i:i + 3]) < 180 for i in range(0, len(pixels), 3))
        luma_levels = len({sum(pixels[i:i + 3]) // 3 for i in range(0, len(pixels), 3)})
        painted = dark >= 200 and luma_levels >= 16
        journal.write("screenshot", path=file.name, sha256=hashlib.sha256(file.read_bytes()).hexdigest(), bytes=file.stat().st_size,
                      content_pixels_verified=painted, dark_content_pixels=dark, luminance_levels=luma_levels)
        if not painted:
            raise AssertionError(f"Actual {label} window is blank or lacks rendered artwork: dark={dark}, luminance_levels={luma_levels}")

    def action(label, operation, predicate):
        before = observed()[-1]["state"]
        journal.write("action-start", action=label, before=before)
        tick = time.monotonic()
        operation()
        after = wait_state(lambda state: predicate(before, state), label)
        journal.write("action-observed", action=label, before=before, after=after,
                      observation_latency_ms=(time.monotonic() - tick) * 1000)
        return after

    def pan(state):
        return (state.get("pan_x", state.get("viewport_x", 0)), state.get("pan_y", state.get("viewport_y", 0)))

    def moved(before, after):
        return any(abs(a - b) > 1 for a, b in zip(pan(before), pan(after)))

    try:
        deadline = started + args.ready_timeout
        while time.monotonic() < deadline:
            try:
                windows = xdo("search", "--onlyvisible", "--pid", process.pid).splitlines()
                if not windows:
                    windows = xdo("search", "--onlyvisible", "--name", "^creditia$").splitlines()
                window = windows[-1]
                break
            except (subprocess.CalledProcessError, IndexError):
                if process.poll() is not None:
                    raise RuntimeError(f"Application exited with {process.returncode} before creating its window")
                time.sleep(.1)
        if not window:
            raise TimeoutError("Application did not create a visible window")
        xdo("windowsize", "--sync", window, 1360, 880)
        xdo("windowactivate", "--sync", window)
        ready = wait_state(lambda s: s.get("ready") is True, "decoded first viewport", max(1, deadline - time.monotonic()))
        if ready.get("style") != "minimal-light" or ready.get("named") is not False or ready.get("view") != "wall":
            raise AssertionError(f"Unexpected initial preferences: {ready}")
        journal.write("ready-observed", startup_ms=(time.monotonic() - started) * 1000, state=ready,
                      window_geometry=xdo("getwindowgeometry", "--shell", window))
        snapshot("ready")
        xdo("mousemove", "--window", window, 900, 450)
        if args.scenario == "startup":
            pass
        elif args.scenario == "scroll":
            for step in range(4):
                action(f"wall-scroll-{step}", lambda: xdo("click", "--repeat", 5, "--delay", 70, 5 if step % 2 == 0 else 4), moved)
        elif args.scenario == "roam":
            action("enter-roam", lambda: xdo("key", "--clearmodifiers", "ctrl+2"), lambda b, s: s.get("view") == "roam")
            for step in range(4):
                action(f"roam-pan-{step}", lambda: xdo("click", "--repeat", 5, "--delay", 70, 5 if step % 2 == 0 else 4), moved)
        elif args.scenario == "zoom":
            # Old Creditia has no continuous zoom. Both real apps open the same
            # selected artwork in their enlarged focus view, then close it.
            for step in range(3):
                state = observed()[-1]["state"]
                point = state.get("first_tile")
                if not point:
                    raise AssertionError("The app did not expose a visible artwork hit target")
                def focus():
                    xdo("mousemove", "--window", window, round(point["x"]), round(point["y"]))
                    xdo("click", "--repeat", 2 if args.application == "gpui" else 1, "--delay", 90, 1)
                action(f"focus-open-{step}", focus, lambda b, s: s.get("focused") is True and s.get("focus_ready") is True)
                if step == 0:
                    snapshot("focus")
                action(f"focus-close-{step}", lambda: xdo("key", "Escape"), lambda b, s: s.get("focused") is False)
        elif args.scenario == "idle":
            journal.write("idle-start", seconds=args.idle_seconds)
            time.sleep(args.idle_seconds)
            wait_state(lambda s: s.get("ready") is True, "visible idle remains ready")
            journal.write("idle-end")
        elif args.scenario == "hidden-idle":
            def os_state():
                props = subprocess.run(["xprop", "-id", str(window), "WM_STATE", "_NET_WM_STATE"], check=True, text=True, capture_output=True).stdout
                geometry = subprocess.run(["xwininfo", "-id", str(window)], check=True, text=True, capture_output=True).stdout
                return {"mapped": "Map State: IsViewable" in geometry, "iconic": "Iconic" in props or "_NET_WM_STATE_HIDDEN" in props}
            def window_action(label, operation, predicate):
                before = os_state(); journal.write("action-start", action=label, before=before, observation_source="x11-window-manager")
                operation(); deadline = time.monotonic() + 15
                while time.monotonic() < deadline:
                    after = os_state()
                    if predicate(after):
                        journal.write("action-observed", action=label, before=before, after=after, observation_source="x11-window-manager")
                        return
                    time.sleep(.1)
                raise TimeoutError(f"Window manager did not observe {label}: {after}")
            # Minimized applications intentionally stop drawing/requestAnimationFrame;
            # waiting for render-time telemetry here would reject correct pausing.
            window_action("hide-window", lambda: xdo("windowminimize", window), lambda s: s["iconic"] or not s["mapped"])
            journal.write("idle-start", seconds=args.idle_seconds)
            time.sleep(args.idle_seconds)
            journal.write("idle-end")
            window_action("restore-window", lambda: (xdo("windowmap", window), xdo("windowactivate", "--sync", window)),
                          lambda s: s["mapped"] and not s["iconic"])
        elif args.scenario == "continuous-zoom":
            if args.application != "gpui":
                raise ValueError("The baseline has no continuous zoom; do not invent a comparable workload")
            for step in range(4):
                action(f"continuous-zoom-{step}", lambda: xdo("key", "--clearmodifiers", "ctrl+equal" if step % 2 == 0 else "ctrl+minus"),
                       lambda b, s: s.get("zoom") != b.get("zoom"))
        else:
            raise ValueError(f"Unsupported scenario: {args.scenario}")
        snapshot("complete")
        journal.write("scenario-complete", scenario=args.scenario, success=True, state=observed()[-1]["state"])
        return 0
    except Exception as error:
        journal.write("scenario-failed", scenario=args.scenario, success=False, error=str(error))
        raise
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def parser():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--application", choices=["tauri", "gpui"], required=True)
    p.add_argument("--fixture", required=True)
    p.add_argument("--scenario", choices=["startup", "scroll", "roam", "zoom", "idle", "hidden-idle", "continuous-zoom"], required=True)
    p.add_argument("--cache", choices=["cold", "warm"], required=True)
    p.add_argument("--ready-timeout", type=float, default=90)
    p.add_argument("--idle-seconds", type=float, default=12)
    p.add_argument("command", nargs=argparse.REMAINDER)
    return p


if __name__ == "__main__":
    raise SystemExit(run(parser().parse_args()))
