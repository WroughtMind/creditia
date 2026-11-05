#!/usr/bin/env python3
"""Sample a launched application's whole process tree, without extra packages.

RSS is the sum of process resident sets, not unique physical memory or GPU memory.
Short-lived children can exit between samples; the report explicitly records this
limit. Never substitute a missing GPU/latency measurement with zero.
"""
import argparse
import datetime as dt
import hashlib
import json
import importlib.util
import os
import pathlib
import platform
import signal
import subprocess
import sys
import time


def sha256(file):
    h = hashlib.sha256()
    with open(file, "rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def duration(value):
    days = 0
    if "-" in value:
        raw, value = value.split("-", 1)
        days = int(raw)
    parts = [float(p) for p in value.split(":")]
    total = 0
    for number in parts:
        total = total * 60 + number
    return days * 86400 + total


def processes():
    """Return process identities and lifetime CPU seconds; no command strings."""
    rows = {}
    if sys.platform.startswith("linux"):
        ticks = os.sysconf("SC_CLK_TCK")
        pages = os.sysconf("SC_PAGE_SIZE")
        for entry in pathlib.Path("/proc").iterdir():
            if not entry.name.isdigit():
                continue
            try:
                raw = (entry / "stat").read_text()
                # comm is parenthesized and may contain spaces and parentheses.
                fields = raw[raw.rfind(")") + 2:].split()
                pid = int(entry.name)
                rows[pid] = {"pid": pid, "ppid": int(fields[1]), "identity": fields[19],
                             "cpu_seconds": (int(fields[11]) + int(fields[12])) / ticks,
                             "rss_bytes": max(0, int(fields[21])) * pages}
            except (OSError, ValueError, IndexError):
                continue  # The process exited while reading its own entry.
    elif sys.platform == "darwin":
        result = subprocess.run(["ps", "-axo", "pid=,ppid=,rss=,time=,lstart="], text=True, capture_output=True, check=True)
        for line in result.stdout.splitlines():
            fields = line.strip().split(maxsplit=4)
            if len(fields) != 5:
                continue
            pid, ppid, rss, cpu, identity = fields
            rows[int(pid)] = {"pid": int(pid), "ppid": int(ppid), "identity": identity,
                              "rss_bytes": int(rss) * 1024, "cpu_seconds": duration(cpu)}
    elif sys.platform == "win32":
        script = "$ErrorActionPreference='Stop'; @(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,CreationDate,WorkingSetSize,KernelModeTime,UserModeTime) | ConvertTo-Json -Compress"
        result = subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", script], text=True, capture_output=True, check=True)
        values = json.loads(result.stdout)
        if isinstance(values, dict):
            values = [values]
        for p in values:
            pid = int(p["ProcessId"])
            rows[pid] = {"pid": pid, "ppid": int(p["ParentProcessId"]), "identity": str(p["CreationDate"]),
                         "rss_bytes": int(p["WorkingSetSize"] or 0),
                         "cpu_seconds": (int(p["KernelModeTime"] or 0) + int(p["UserModeTime"] or 0)) / 10000000}
    else:
        raise RuntimeError(f"Unsupported process sampler platform: {sys.platform}")
    return rows


def descendants(root, table, retained):
    selected = {pid for pid, identity in retained.items() if pid in table and table[pid]["identity"] == identity}
    if root in table:
        selected.add(root)
    while True:
        children = {pid for pid, row in table.items() if row["ppid"] in selected}
        if children <= selected:
            return selected
        selected |= children


def percentile(values, fraction):
    if not values:
        return None
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, round((len(ordered) - 1) * fraction))]


def run(args):
    command = args.command[1:] if args.command and args.command[0] == "--" else args.command
    if not command:
        raise ValueError("A program and arguments are required after --")
    output = pathlib.Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    fixture = pathlib.Path(args.fixture).resolve()
    if not fixture.is_file():
        raise ValueError("A pinned input fixture is required")
    if args.interval < .05 or args.seconds <= 0:
        raise ValueError("Interval must be >= 0.05 seconds and duration must be positive")
    if sys.platform.startswith("linux"):
        # Some managed executors virtualize subprocess PIDs without mounting a
        # matching /proc. In that environment a small Popen PID can identify an
        # unrelated host process. Refuse both observation and cleanup before
        # launching anything; never publish those numbers as measurements.
        proc_pid = int(pathlib.Path("/proc/self/stat").read_text().split(" ", 1)[0])
        if proc_pid != os.getpid():
            raise RuntimeError("Process IDs and /proc use different namespaces; resource sampling is unavailable in this executor")
    metadata = {"schema_version": 1, "kind": "performance", "scenario": args.scenario, "cache_state": args.cache,
                "comparison_role": args.comparison_role,
                "provenance": {"candidate_sha": args.commit, "environment": {"platform": sys.platform,
                    "arch": platform.machine(), "release": platform.release(), "cpu": platform.processor(),
                    "logical_cpus": os.cpu_count(), "rendering": {key: os.environ.get(key) for key in ["LIBGL_ALWAYS_SOFTWARE", "WEBKIT_DISABLE_COMPOSITING_MODE", "GDK_BACKEND", "WGPU_BACKEND", "VK_ICD_FILENAMES"]}, "machine_id_sha256": hashlib.sha256(platform.node().encode()).hexdigest()},
                    "measured_commit": args.measured_commit or args.commit,
                    "fixture": {"sha256": sha256(fixture), "name": fixture.name},
                    "workload": {"styles": args.styles, "artworks": args.artworks, "window": args.window, "dpi": args.dpi}},
                "started_at": dt.datetime.now(dt.timezone.utc).isoformat(), "sample_interval_seconds": args.interval,
                "limits": ["Summed RSS can count shared pages more than once; it is not PSS or unique memory.",
                    "Children that exit between samples can be missed; sampled CPU is a lower bound.",
                    "GPU bytes, energy and interaction latency are not inferred from CPU/RSS.",
                    "Cold/warm is a workload label; this script never flushes operating-system caches."]}
    env = dict(os.environ)
    env["CREDITIA_PERF_EVENTS"] = str(output / "events.jsonl")
    stdout = open(output / "stdout.log", "wb")
    stderr = open(output / "stderr.log", "wb")
    process = subprocess.Popen(command, env=env, stdout=stdout, stderr=stderr,
                               start_new_session=sys.platform != "win32")
    start = time.monotonic()
    retained, high_water, samples = {}, {}, []
    previous_total = 0.0
    previous_time = start
    timed_out = False
    try:
        with open(output / "samples.jsonl", "w") as file:
            while True:
                now = time.monotonic()
                table = processes()
                pids = descendants(process.pid, table, retained)
                for pid in pids:
                    row = table[pid]
                    retained[pid] = row["identity"]
                    identity = f"{pid}:{row['identity']}"
                    high_water[identity] = max(high_water.get(identity, 0.0), row["cpu_seconds"])
                total = sum(high_water.values())
                elapsed = now - previous_time
                sample = {"elapsed_seconds": now - start, "processes": [table[pid] for pid in sorted(pids)],
                          "monotonic_seconds": now, "rss_bytes": sum(table[pid]["rss_bytes"] for pid in pids),
                          "observed_cpu_seconds": total,
                          "cpu_percent_one_core": max(0.0, (total - previous_total) / elapsed * 100) if elapsed > 0 else None}
                samples.append(sample)
                file.write(json.dumps(sample) + "\n")
                file.flush()
                previous_total, previous_time = total, now
                if process.poll() is not None and not pids:
                    break
                if now - start >= args.seconds:
                    timed_out = True
                    break
                time.sleep(min(args.interval, max(0, args.seconds - (time.monotonic() - start))))
    finally:
        current = processes()
        remaining = descendants(process.pid, current, retained)
        metadata["remaining_processes_at_end"] = len(remaining)
        if process.poll() is None or remaining:
            if sys.platform == "win32":
                for pid in sorted(remaining):
                    subprocess.run(["taskkill", "/PID", str(pid), "/T", "/F"], capture_output=True)
            else:
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                if sys.platform != "win32":
                    os.killpg(process.pid, signal.SIGKILL)
                process.kill()
                process.wait()
        stdout.close()
        stderr.close()
    # Ignore the first CPU sample (lifetime CPU / a nearly zero interval).
    cpu = [s["cpu_percent_one_core"] for s in samples[1:] if s["cpu_percent_one_core"] is not None]
    rss = [s["rss_bytes"] for s in samples]
    events = []
    event_file = output / "events.jsonl"
    if event_file.exists():
        for line in event_file.read_text().splitlines():
            try:
                value = json.loads(line)
                if not isinstance(value, dict):
                    raise ValueError("Event must be an object")
                events.append(value)
            except (json.JSONDecodeError, ValueError) as error:
                metadata.setdefault("event_errors", []).append(str(error))
    spec = importlib.util.spec_from_file_location("creditia_scenario", pathlib.Path(__file__).with_name("scenario.py"))
    scenario = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(scenario)
    chain_verified = scenario.verify_journal(events)
    workload_verified = chain_verified and any(event.get("event") == "scenario-complete" and event.get("scenario") == args.scenario
                            and event.get("success") is True for event in events)
    phase_summary = {}
    for phase, begin, end in [("startup", "launch", "ready-observed"), ("idle", "idle-start", "idle-end")]:
        first = next((e for e in events if e.get("event") == begin), None)
        last = next((e for e in events if e.get("event") == end), None)
        if first and last and "monotonic_seconds" in first and "monotonic_seconds" in last:
            selected = [sample for sample in samples if first["monotonic_seconds"] <= sample["monotonic_seconds"] <= last["monotonic_seconds"]]
            phase_summary[phase] = {"duration_seconds": last["monotonic_seconds"] - first["monotonic_seconds"],
                "sample_count": len(selected), "rss_peak_bytes": max((row["rss_bytes"] for row in selected), default=None),
                "cpu_p50_percent_one_core": percentile([row["cpu_percent_one_core"] for row in selected], .5)}
    ready = next((event for event in events if event.get("event") == "ready-observed"), None)
    if ready:
        actual_count = ready.get("state", {}).get("tile_count")
        metadata["provenance"]["workload"]["observed_artworks"] = actual_count
        if actual_count != args.artworks:
            workload_verified = False
            metadata.setdefault("event_errors", []).append(f"Expected {args.artworks} artwork slots, observed {actual_count}")
    metadata["provenance"]["workload"]["action_definition"] = "focus-enlargement" if args.scenario == "zoom" else args.scenario
    metadata["provenance"]["workload"]["cache_definition"] = "fresh isolated app profile; OS page cache uncontrolled" if args.cache == "cold" else "same profile after a complete first-viewport launch; process memory reset"
    metadata["provenance"]["driver_sha256"] = sha256(pathlib.Path(__file__).with_name("scenario.py"))
    metadata.update({"completed_at": dt.datetime.now(dt.timezone.utc).isoformat(), "complete": True,
                     "workload_verified": workload_verified, "event_chain_verified": chain_verified, "phase_summary": phase_summary,
                     "exit_code": process.returncode, "ended_by_sampler": timed_out,
                     "summary": {"sample_count": len(samples), "observed_process_count": len(high_water),
                        "rss_peak_bytes": max(rss, default=0), "rss_p50_bytes": percentile(rss, .5),
                        "cpu_p50_percent_one_core": percentile(cpu, .5), "cpu_p95_percent_one_core": percentile(cpu, .95),
                        "observed_cpu_seconds": sum(high_water.values()), "gpu_memory_bytes": None,
                        "interaction_latency_ms": None, "app_events": events}, "artifacts": []})
    for name in ["samples.jsonl", "stdout.log", "stderr.log", "events.jsonl", "application-events.jsonl", "ready.png", "focus.png", "complete.png"]:
        p = output / name
        if p.exists():
            metadata["artifacts"].append({"path": name, "sha256": sha256(p), "bytes": p.stat().st_size})
    (output / "manifest.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(json.dumps({"output": str(output), "workload_verified": workload_verified, "summary": {k: v for k, v in metadata["summary"].items() if k != "app_events"}, "exit_code": process.returncode}))
    # Stopping a still-running UI at the configured deadline is expected.
    return 0 if workload_verified and process.returncode == 0 and not timed_out else 1


def parser():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--output", required=True)
    p.add_argument("--fixture", required=True)
    p.add_argument("--commit", required=True)
    p.add_argument("--measured-commit", help="Commit of the measured binary; --commit identifies the candidate being accepted")
    p.add_argument("--comparison-role", choices=["baseline", "candidate"], required=True)
    p.add_argument("--scenario", choices=["startup", "scroll", "roam", "zoom", "idle", "hidden-idle", "continuous-zoom"], required=True)
    p.add_argument("--cache", choices=["cold", "warm"], required=True)
    p.add_argument("--styles", type=int, required=True)
    p.add_argument("--artworks", type=int, required=True)
    p.add_argument("--window", default="1360x880")
    p.add_argument("--dpi", type=float, default=1)
    p.add_argument("--seconds", type=float, default=30)
    p.add_argument("--interval", type=float, default=.5)
    p.add_argument("command", nargs=argparse.REMAINDER)
    return p


if __name__ == "__main__":
    raise SystemExit(run(parser().parse_args()))
