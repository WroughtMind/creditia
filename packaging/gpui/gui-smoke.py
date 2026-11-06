#!/usr/bin/env python3
"""Open the installed application with its own renderer, fonts and runtime.

The archive is fixed test data; all view construction, layout, native image
decoding and rendering come from the shipped application. No checkout resource
override is passed to it. Screenshots and normal quit are observed through OS
windows, separately from the application's readiness events.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time

from PIL import Image


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def last_events(path):
    events = []
    if path.exists():
        for line in path.read_text(errors="replace").splitlines():
            try:
                events.append(json.loads(line))
            except json.JSONDecodeError:
                pass  # The last append may still be in progress.
    state = next((x["state"] for x in reversed(events) if x.get("event") == "state"), {})
    ready = next((x for x in reversed(events) if x.get("event") == "ready"), {})
    return ready, state


def pixels(path):
    with Image.open(path) as image:
        image.load()
        if image.width < 800 or image.height < 500:
            raise RuntimeError(f"Installed window has unexpected dimensions: {image.size}")
        # Exclude titlebar: an empty application with a window decoration is not
        # successful rendering. The retained original permits visual inspection.
        body = image.convert("RGB").crop((0, 70, image.width, image.height - 20))
        colors = len(set(body.resize((320, 200)).getdata()))
        if colors < 64:
            raise RuntimeError(f"Installed application pixels are blank ({colors} colors)")
        return {"width": image.width, "height": image.height, "sampled_colors": colors}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--archive", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--inside-x11", action="store_true")
    args = parser.parse_args()
    if sys.platform == "linux" and not args.inside_x11:
        command = ["xvfb-run", "-a", "-s", "-screen 0 1600x1000x24", sys.executable,
                   str(Path(__file__).resolve()), *sys.argv[1:], "--inside-x11"]
        return subprocess.run(command, timeout=240).returncode
    if sys.platform not in ("linux", "darwin"):
        raise RuntimeError("This installed GUI check is for macOS and Linux")
    binary, archive, output = Path(args.binary).resolve(), Path(args.archive).resolve(), Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    report = {"schema_version": 1, "kind": "installed-native-window", "complete": False,
              "provenance": {"candidate_sha": args.candidate, "platform": sys.platform,
                             "binary_sha256": sha(binary), "fixture_sha256": sha(archive)},
              "checks": [], "artifacts": []}
    process = wm = None
    logs = []
    with tempfile.TemporaryDirectory(prefix="creditia-installed-window-") as temporary:
        work = Path(temporary)
        fixture = work / "archive.json"
        fixture.write_bytes(archive.read_bytes())
        events = output / "events.jsonl"
        events.unlink(missing_ok=True)
        env = dict(os.environ)
        for key in ("CREDITIA_CORE", "CREDITIA_NODE", "CREDITIA_RENDER_BRIDGE", "CREDITIA_PERF_EVENTS", "NODE_PATH", "NODE_OPTIONS", "WAYLAND_DISPLAY"):
            env.pop(key, None)
        env.update(CREDITIA_HOME=str(work / "library"), CREDITIA_APP_EVENTS=str(events))
        try:
            def run(command, timeout=10, check=True):
                return subprocess.run(command, cwd=work, env=env, capture_output=True, text=True, timeout=timeout, check=check)

            if sys.platform == "linux":
                runtime = work / "runtime"
                runtime.mkdir(mode=0o700)
                env.update(XDG_RUNTIME_DIR=str(runtime), XDG_SESSION_TYPE="x11")
                wm_log = (output / "window-manager.log").open("wb")
                logs.append(wm_log)
                wm = subprocess.Popen(["openbox"], cwd=work, env=env, stdout=wm_log, stderr=wm_log)
                deadline = time.monotonic() + 10
                while "window id" not in run(["xprop", "-root", "_NET_SUPPORTING_WM_CHECK"], check=False).stdout:
                    if wm.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError("Installed-window X11 manager did not become ready")
                    time.sleep(.1)
            else:
                helper = work / "gui-window"
                run(["swiftc", str(Path(__file__).with_name("gui-window.swift")), "-o", str(helper)], timeout=90)

            for name in ("stdout.log", "stderr.log"):
                logs.append((output / name).open("wb"))
            process = subprocess.Popen([str(binary), "--benchmark-archive", str(fixture)], cwd=work,
                                       env=env, stdout=logs[-2], stderr=logs[-1], start_new_session=True)
            window = None
            deadline = time.monotonic() + 150
            ready = state = {}
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError(f"Installed application exited before painting: {process.returncode}")
                if window is None:
                    if sys.platform == "linux":
                        found = run(["xdotool", "search", "--onlyvisible", "--pid", str(process.pid)], check=False)
                        if found.returncode == 0 and found.stdout.strip():
                            window = found.stdout.strip().splitlines()[-1]
                            run(["xdotool", "windowactivate", "--sync", window])
                    else:
                        run([str(helper), "activate", str(process.pid)], check=False)
                        found = run([str(helper), "window", str(process.pid)], check=False)
                        if found.returncode == 0:
                            window = str(json.loads(found.stdout)["id"])
                ready, state = last_events(events)
                if window and ready.get("loaded", 0) > 0 and state.get("ready") and state.get("first_tile"):
                    break
                time.sleep(.15)
            else:
                raise TimeoutError(f"Installed window did not show decoded artwork: {state}")
            screenshot = output / "window.png"
            capture_deadline = time.monotonic() + 12
            while True:
                if sys.platform == "linux":
                    run(["import", "-window", window, str(screenshot)])
                else:
                    run(["screencapture", "-x", "-o", "-l", window, str(screenshot)])
                try:
                    dimensions = pixels(screenshot)
                    break
                except RuntimeError:
                    if time.monotonic() > capture_deadline:
                        raise
                    time.sleep(.2)
            report["checks"].append({"id": "installed-production-window-and-bundled-artwork", "passed": True,
                                     "ready_event": ready, "state": state, **dimensions})
            if sys.platform == "linux":
                run(["xdotool", "windowactivate", "--sync", window])
                run(["xdotool", "key", "--clearmodifiers", "ctrl+q"])
            else:
                run([str(helper), "quit", str(process.pid)])
            code = process.wait(timeout=20)
            if code != 0:
                raise RuntimeError(f"Installed application did not quit normally: {code}")
            report["checks"].append({"id": "installed-native-normal-quit", "passed": True, "exit_code": code})
            report["complete"] = True
        except Exception as error:
            report["error"] = f"{type(error).__name__}: {error}"
        finally:
            if process is not None:
                # Catch renderer descendants even if their parent already quit.
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
                try:
                    wm.terminate()
                    wm.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    wm.kill()
                    wm.wait()
            for log in logs:
                log.close()
            for file in sorted(output.iterdir()):
                if file.is_file() and file.name != "window-evidence.json":
                    report["artifacts"].append({"path": file.name, "bytes": file.stat().st_size, "sha256": sha(file)})
            (output / "window-evidence.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"complete": report["complete"], "error": report.get("error")}))
    return 0 if report["complete"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
