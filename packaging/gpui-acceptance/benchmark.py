#!/usr/bin/env python3
"""Measure GPUI workloads in isolated profiles on the same X11 desktop."""
import argparse
import csv
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time

from scenario import DELIVERY_SCENARIOS, SCENARIOS

HERE = pathlib.Path(__file__).resolve().parent


def retain_warmup(source, destination):
    """Keep the complete warmup proof without treating it as a measurement.

    Relative artifact links in the warmup report still resolve beside that
    report, while the acceptance collector only discovers manifest.json files.
    Failed warmups are retained too, before the isolated profile is removed.
    """
    artifacts = []
    source, destination = pathlib.Path(source), pathlib.Path(destination)
    for original in sorted(source.rglob("*")):
        if not original.is_file():
            continue
        relative = original.relative_to(source)
        if relative == pathlib.Path("manifest.json"):
            relative = pathlib.Path("warmup-report.json")
        target = destination / "warmup" / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(original, target)
        artifacts.append({"path": target.relative_to(destination).as_posix(),
                          "sha256": hashlib.sha256(target.read_bytes()).hexdigest(), "bytes": target.stat().st_size})
    return artifacts


def run(args):
    output = pathlib.Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    fixture = pathlib.Path(args.fixture).resolve()
    binary = pathlib.Path(args.candidate_bin).resolve()
    if not binary.is_file():
        raise ValueError(f"Missing native executable: {binary}")
    binary_sha = hashlib.sha256(binary.read_bytes()).hexdigest()
    scenarios = DELIVERY_SCENARIOS if args.delivery else args.scenarios.split(",")
    matrix = HERE.parent.parent / "docs/gpui-migration/matrix.tsv"
    with matrix.open() as stream:
        expected_artworks = sum(row["style"] == "minimal-light" and row["renderer"] == "classic" and row["desktop"] == "true" for row in csv.DictReader(stream, delimiter="\t"))
    if args.artworks is not None and args.artworks != expected_artworks:
        raise ValueError(f"The complete minimal-light wall contains {expected_artworks} layout slots, not {args.artworks}")
    failures = []
    with tempfile.TemporaryDirectory(prefix="creditia-benchmark-profiles-") as temporary:
        for scenario in scenarios:
            for cache in (("cold",) if args.delivery else ("cold", "warm")):
                profile = pathlib.Path(temporary) / f"{scenario}-{cache}"
                profile.mkdir()
                env = dict(os.environ, XDG_CONFIG_HOME=str(profile / "config"), XDG_DATA_HOME=str(profile / "data"),
                           XDG_CACHE_HOME=str(profile / "cache"), CREDITIA_HOME=str(profile / "creditia"))

                def resource_command(destination, selected_scenario):
                    return [sys.executable, str(HERE / "resources.py"), "--output", str(destination),
                            "--fixture", str(fixture), "--commit", args.candidate_sha,
                            "--scenario", selected_scenario, "--cache", cache,
                            "--styles", "1", "--artworks", str(expected_artworks), "--window", "1360x880", "--dpi", "1",
                            "--seconds", "180", "--interval", ".25", "--",
                            sys.executable, str(HERE / "scenario.py"), "--fixture", str(fixture),
                            "--cache", cache, "--scenario", selected_scenario, "--",
                            str(binary), "--benchmark-archive", str(fixture)]

                destination = output / f"{scenario}-{cache}"
                if destination.exists():
                    raise ValueError(f"Refusing to reuse evidence directory: {destination}")
                warmup_artifacts = []
                if cache == "warm":
                    warmup = profile / "warmup"
                    # Use the same process cleanup as the measurement.
                    result = subprocess.run(resource_command(warmup, "startup"), env=env)
                    warmup_artifacts = retain_warmup(warmup, destination)
                    if result.returncode:
                        failures.append({"scenario": scenario, "cache": cache, "phase": "warmup", "exit_code": result.returncode})
                        continue
                result = subprocess.run(resource_command(destination, scenario), env=env)
                if result.returncode:
                    failures.append({"scenario": scenario, "cache": cache, "phase": "measurement", "exit_code": result.returncode})
                manifest = destination / "manifest.json"
                if manifest.exists():
                    data = json.loads(manifest.read_text())
                    data["provenance"]["binary_sha256"] = binary_sha
                    data["artifacts"].extend(warmup_artifacts)
                    manifest.write_text(json.dumps(data, indent=2) + "\n")
                time.sleep(.3)
    (output / "run-summary.json").write_text(json.dumps({"candidate_sha": args.candidate_sha,
        "binary_sha256": binary_sha, "profile": "delivery" if args.delivery else "full",
        "fixture_sha256": hashlib.sha256(fixture.read_bytes()).hexdigest(), "failures": failures,
        "limitations": ["The measured host uses Linux software rendering.",
                        "Cold uses a fresh app profile. OS page caches are left in place.",
                        "Process totals include the scenario driver and its X11 input subprocesses.",
                        "Hidden-idle measures a static wall. Animation suspension has its own acceptance check."]}, indent=2) + "\n")
    return 1 if failures else 0


def parser():
    p = argparse.ArgumentParser(description=__doc__)
    for option in ["output", "fixture", "candidate-bin", "candidate-sha"]:
        p.add_argument("--" + option, required=True)
    p.add_argument("--artworks", type=int, help="Optional assertion; defaults to the actual complete wall count in matrix.tsv")
    p.add_argument("--delivery", action="store_true", help="Measure four representative workloads from cold app profiles")
    p.add_argument("--scenarios", default=",".join(SCENARIOS))
    return p


if __name__ == "__main__":
    raise SystemExit(run(parser().parse_args()))
