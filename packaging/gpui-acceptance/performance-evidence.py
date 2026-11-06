#!/usr/bin/env python3
"""Bind actual same-host process measurements to their checklist assertions."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import shutil

from scenario import verify_journal
from resources import screenshot_proofs

SCENARIOS = ("startup", "scroll", "roam", "zoom", "idle", "hidden-idle")


def sha(data):
    return hashlib.sha256(data).hexdigest()


def verified_artifact(root, record):
    file = (root / record["path"]).resolve()
    if not file.is_relative_to(root.resolve()):
        raise ValueError("Artifact escaped its measurement directory")
    data = file.read_bytes()
    if sha(data) != record["sha256"] or len(data) != record["bytes"]:
        raise ValueError(f"Artifact differs: {file}")
    return file


def verify_samples(rows, summary):
    if not rows or len(rows) != summary["sample_count"]:
        raise ValueError("Missing raw process samples")
    observed = {}
    for sample in rows:
        if sum(p["rss_bytes"] for p in sample["processes"]) != sample["rss_bytes"]:
            raise ValueError("Process RSS sum differs from the recorded sample")
        for process in sample["processes"]:
            key = (process["pid"], process["identity"])
            observed[key] = max(observed.get(key, 0), process["cpu_seconds"])
        if not math.isclose(sum(observed.values()), sample["observed_cpu_seconds"], abs_tol=1e-7):
            raise ValueError("CPU total differs from observed process identities")
    if (len(observed) != summary["observed_process_count"] or
            max(row["rss_bytes"] for row in rows) != summary["rss_peak_bytes"] or
            not math.isclose(sum(observed.values()), summary["observed_cpu_seconds"], abs_tol=1e-7)):
        raise ValueError("Process summary differs from its raw samples")


def seal(source, output, candidate):
    source, output = Path(source).resolve(), Path(output).resolve()
    if len(candidate) != 40 or any(c not in "0123456789abcdef" for c in candidate):
        raise ValueError("Full candidate commit is required")
    summary = json.loads((source / "run-summary.json").read_text())
    if summary["candidate_sha"] != candidate or summary["failures"]:
        raise ValueError("Performance run failed or belongs to a different candidate")
    manifests, artifacts, pairs = {}, [], []

    def retain(file, relative):
        target = output / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(file, target)
        data = target.read_bytes()
        artifacts.append({"path": relative, "sha256": sha(data), "bytes": len(data)})

    delivery = summary.get("profile") == "delivery"
    scenarios = ("startup", "scroll", "roam", "hidden-idle") if delivery else SCENARIOS
    for scenario in scenarios:
        for cache in (("cold",) if delivery else ("cold", "warm")):
            pair = []
            for role in ("baseline", "candidate"):
                key = f"{role}-{scenario}-{cache}"
                root = source / key
                file = root / "manifest.json"
                data = json.loads(file.read_text())
                if not (data["kind"] == "performance" and data["complete"] is True
                        and data["scenario"] == scenario and data["cache_state"] == cache
                        and data["comparison_role"] == role and data["exit_code"] == 0
                        and data["workload_verified"] is True and data["event_chain_verified"] is True
                        and data["screenshot_content_verified"] is True
                        and data["remaining_processes_at_end"] == 0
                        and data["provenance"]["candidate_sha"] == candidate
                        and data["provenance"]["fixture"]["sha256"] == summary["fixture_sha256"]
                        and data["provenance"]["measured_commit"] == (candidate if role == "candidate" else summary["baseline_sha"])):
                    raise ValueError(f"Incomplete or inconsistent measurement: {key}")
                for record in data["artifacts"]:
                    retain(verified_artifact(root, record), f"runs/{key}/{record['path']}")
                events = [json.loads(line) for line in (root / "events.jsonl").read_text().splitlines()]
                if not verify_journal(events) or events != data["summary"]["app_events"]:
                    raise ValueError(f"Recorded action chain differs: {key}")
                if not screenshot_proofs(events, root)["verified"]:
                    raise ValueError(f"Painted-window proof differs: {key}")
                samples = [json.loads(line) for line in (root / "samples.jsonl").read_text().splitlines()]
                verify_samples(samples, data["summary"])
                if not any(event.get("event") == "ready-observed" and event.get("startup_ms", -1) >= 0 for event in events):
                    raise ValueError(f"No observed startup time: {key}")
                if scenario in ("scroll", "roam", "zoom") and not any(
                        event.get("event") == "action-observed" and event.get("observation_latency_ms", -1) >= 0 for event in events):
                    raise ValueError(f"No observed input latency: {key}")
                retain(file, f"runs/{key}/measurement.json")
                manifests[key] = data
                pair.append(data)
            def identity(data):
                p = data["provenance"]
                return {"environment": p["environment"], "fixture": p["fixture"]["sha256"], "workload": p["workload"]}
            if identity(pair[0]) != identity(pair[1]):
                raise ValueError(f"Baseline and candidate workloads differ: {scenario}/{cache}")
            pairs.append({"scenario": scenario, "cache": cache, **identity(pair[0])})
    retain(source / "run-summary.json", "run-summary.json")
    feature = "new-performance"
    cases = [
        {"key": "same-host-same-input", "feature_id": feature, "check": "相同设备、数据、作品和布局规模",
         "test_id": "paired-production-workloads", "status": "passed",
         "behavior": "All recorded old/new pairs match machine fingerprint, exact public archive hash, viewport, DPI and actual 68-slot wall; input journals and painted-window proofs validate."},
        {"key": "observed-common-workloads", "feature_id": feature, "check": "冷/热启动、滚动、缩放、交互延迟",
         "test_id": "paired-production-workloads", "status": "not_run" if delivery else "passed",
         "behavior": "The full profile measures cold/warm startup, scrolling, panning and focus enlargement on both apps. Reported observation latency includes driver polling, not presentation latency; GPUI continuous zoom has its separate runtime test."},
        {"key": "whole-process-rss-cpu", "feature_id": feature, "check": "整组进程 RSS 与 CPU 资源",
         "test_id": "raw-process-sample-replay", "status": "passed",
         "behavior": "Replays every per-process sample, checking PID/start identities, summed RSS and cumulative observed CPU against published totals, and verifies no measured processes remain. GPU telemetry is explicitly unmeasured."},
        {"key": "measured-idle-needs-animation-proof", "feature_id": feature, "check": "空闲 CPU 与不可见动画",
         "test_id": "performance-idle-cpu + native-hidden-animation", "status": "not_run",
         "behavior": "Visible and hidden static-wall idle CPU is measured for both apps; acceptance additionally requires the independent installed native animation suspension assertion.",
         "tests": [{"test_id": "performance-idle-cpu", "outcomes": [{"status": "passed", "source": "recorded hidden-idle process samples"}]},
                   {"test_id": "native-hidden-animation", "outcomes": []}]},
    ]
    result = {"schema_version": 1, "kind": "feature-checks", "complete": True,
              "provenance": {"candidate_sha": candidate, "source": "Revalidated same-host production performance artifacts"},
              "cases": cases, "artifacts": artifacts, "pairs": pairs,
              "scope": {"platform": "Linux software graphics", "styles": 1, "layout_slots": 68,
                        "limits": summary["limitations"] + ["GPU telemetry is unmeasured; this is not a hardware-independent performance claim."]}}
    (output / "manifest.json").write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--candidate-sha", required=True)
    args = parser.parse_args()
    result = seal(args.input, args.output, args.candidate_sha)
    print(json.dumps({"output": args.output, "pairs": len(result["pairs"]), "cases": len(result["cases"])}))
