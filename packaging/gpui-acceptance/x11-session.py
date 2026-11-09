#!/usr/bin/env python3
"""Keep one window manager for sequential baseline/candidate measurements."""
import os
import pathlib
import subprocess
import sys
import time

command = sys.argv[1:]
if command[:1] == ["--"]:
    command = command[1:]
if not command:
    raise SystemExit("Pass the comparison command after --")
log = pathlib.Path(os.environ.get("RUNNER_TEMP", "/tmp")) / "creditia-openbox.log"
with log.open("wb") as output:
    wm = subprocess.Popen(["openbox"], stdout=output, stderr=output)
    try:
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            state = subprocess.run(["xprop", "-root", "_NET_SUPPORTING_WM_CHECK"], capture_output=True, text=True)
            if "window id" in state.stdout:
                break
            if wm.poll() is not None:
                raise RuntimeError("The X11 window manager exited before readiness")
            time.sleep(.1)
        else:
            raise TimeoutError("The X11 window manager did not become ready")
        result = subprocess.run(command)
    finally:
        wm.terminate()
        wm.wait(timeout=5)
raise SystemExit(result.returncode)
