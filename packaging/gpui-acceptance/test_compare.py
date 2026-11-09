import hashlib
import importlib.util
import json
import pathlib
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("compare", pathlib.Path(__file__).with_name("compare.py"))
compare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compare)


class WarmupEvidenceTests(unittest.TestCase):
    def test_retains_failed_warmup_files_with_resolvable_links(self):
        with tempfile.TemporaryDirectory() as directory:
            source, destination = pathlib.Path(directory) / "profile", pathlib.Path(directory) / "evidence"
            source.mkdir()
            image = b"actual screenshot bytes"
            (source / "ready.png").write_bytes(image)
            (source / "events.jsonl").write_text('{"event":"scenario-failed"}\n')
            (source / "manifest.json").write_text(json.dumps({"workload_verified": False, "artifacts": [
                {"path": "ready.png", "sha256": hashlib.sha256(image).hexdigest(), "bytes": len(image)}]}))
            artifacts = compare.retain_warmup(source, destination)
            self.assertFalse(list(destination.rglob("manifest.json")))
            report = destination / "warmup/warmup-report.json"
            self.assertFalse(json.loads(report.read_text())["workload_verified"])
            for artifact in artifacts:
                file = destination / artifact["path"]
                self.assertEqual(hashlib.sha256(file.read_bytes()).hexdigest(), artifact["sha256"])
                self.assertEqual(file.stat().st_size, artifact["bytes"])
            linked_image = report.parent / json.loads(report.read_text())["artifacts"][0]["path"]
            self.assertEqual(linked_image.read_bytes(), image)


if __name__ == "__main__":
    unittest.main()
