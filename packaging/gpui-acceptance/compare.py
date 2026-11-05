#!/usr/bin/env python3
"""Run matching production workloads sequentially on the same X11 desktop."""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time

HERE = pathlib.Path(__file__).resolve().parent


def run(args):
    output = pathlib.Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    fixture = pathlib.Path(args.fixture).resolve()
    binaries = {"baseline": pathlib.Path(args.baseline_bin).resolve(), "candidate": pathlib.Path(args.candidate_bin).resolve()}
    for binary in binaries.values():
        if not binary.is_file():
            raise ValueError(f"Missing native executable: {binary}")
    scenarios = args.scenarios.split(",")
    failures = []
    with tempfile.TemporaryDirectory(prefix="creditia-benchmark-profiles-") as temporary:
        for scenario_index, scenario in enumerate(scenarios):
            for cache in ["cold", "warm"]:
                # Alternate launch order to make one implementation less likely
                # to benefit systematically from the host's changing load.
                roles = ["baseline", "candidate"] if scenario_index % 2 == 0 else ["candidate", "baseline"]
                for role in roles:
                    application = "tauri" if role == "baseline" else "gpui"
                    profile = pathlib.Path(temporary) / f"{role}-{scenario}-{cache}"
                    profile.mkdir()
                    env = dict(os.environ, XDG_CONFIG_HOME=str(profile / "config"), XDG_DATA_HOME=str(profile / "data"),
                               XDG_CACHE_HOME=str(profile / "cache"), CREDITIA_HOME=str(profile / "creditia"))
                    command = [str(binaries[role])]
                    if role == "candidate":
                        command += ["--benchmark-archive", str(fixture)]
                    base = [sys.executable, str(HERE / "scenario.py"), "--application", application,
                            "--fixture", str(fixture), "--cache", cache]
                    def resource_command(destination, selected_scenario):
                        return [sys.executable, str(HERE / "resources.py"), "--output", str(destination),
                                "--fixture", str(fixture), "--commit", args.candidate_sha,
                                "--measured-commit", args.baseline_sha if role == "baseline" else args.candidate_sha,
                                "--comparison-role", role, "--scenario", selected_scenario, "--cache", cache,
                                "--styles", "1", "--artworks", str(args.artworks), "--window", "1360x880", "--dpi", "1",
                                "--seconds", "180", "--interval", ".25", "--",
                                *base, "--scenario", selected_scenario, "--", *command]
                    if cache == "warm":
                        warmup = profile / "warmup"
                        # Warmup uses the same process-group cleanup as the
                        # measured run; orphan workers must not contaminate it.
                        result = subprocess.run(resource_command(warmup, "startup"), env=env)
                        if result.returncode:
                            failures.append({"role": role, "scenario": scenario, "cache": cache, "phase": "warmup", "exit_code": result.returncode})
                            continue
                    destination = output / f"{role}-{scenario}-{cache}"
                    if destination.exists():
                        raise ValueError(f"Refusing to reuse evidence directory: {destination}")
                    resource = resource_command(destination, scenario)
                    result = subprocess.run(resource, env=env)
                    if result.returncode:
                        failures.append({"role": role, "scenario": scenario, "cache": cache, "phase": "measurement", "exit_code": result.returncode})
                    manifest = destination / "manifest.json"
                    if manifest.exists():
                        data = json.loads(manifest.read_text())
                        data["provenance"]["binary_sha256"] = hashlib.sha256(binaries[role].read_bytes()).hexdigest()
                        if role == "baseline":
                            overlay = pathlib.Path(args.baseline_overlay)
                            target = destination / "baseline-overlay.json"
                            target.write_bytes(overlay.read_bytes())
                            data["artifacts"].append({"path": target.name, "sha256": hashlib.sha256(target.read_bytes()).hexdigest(), "bytes": target.stat().st_size})
                        if cache == "warm":
                            for name in ["events.jsonl", "application-events.jsonl", "manifest.json"]:
                                original = profile / "warmup" / name
                                if original.exists():
                                    target = destination / ("warmup-report.json" if name == "manifest.json" else f"warmup-{name}")
                                    target.write_bytes(original.read_bytes())
                                    data["artifacts"].append({"path": target.name, "sha256": hashlib.sha256(target.read_bytes()).hexdigest(), "bytes": target.stat().st_size})
                        manifest.write_text(json.dumps(data, indent=2) + "\n")
                    time.sleep(.3)
    (output / "run-summary.json").write_text(json.dumps({"candidate_sha": args.candidate_sha, "baseline_sha": args.baseline_sha,
        "fixture_sha256": hashlib.sha256(fixture.read_bytes()).hexdigest(), "failures": failures,
        "limitations": ["Linux software rendering on this host only; no universal hardware performance claim.",
                        "The cold label means a fresh app profile; operating-system page caches are not purged.",
                        "Both root process groups include the same scenario driver and X11 input subprocesses.",
                        "zoom means enlarged focus view; the original app does not implement continuous zoom.",
                        "hidden-idle compares hiding a static production wall; candidate animation behavior is a separate acceptance check."]}, indent=2) + "\n")
    return 1 if failures else 0


def parser():
    p = argparse.ArgumentParser(description=__doc__)
    for option in ["output", "fixture", "baseline-bin", "candidate-bin", "baseline-sha", "candidate-sha", "baseline-overlay"]:
        p.add_argument("--" + option, required=True)
    p.add_argument("--artworks", type=int, default=27)
    p.add_argument("--scenarios", default="startup,scroll,roam,zoom,idle,hidden-idle")
    return p


if __name__ == "__main__":
    raise SystemExit(run(parser().parse_args()))
