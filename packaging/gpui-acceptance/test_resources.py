import importlib.util
import pathlib
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("resources", pathlib.Path(__file__).with_name("resources.py"))
resources = importlib.util.module_from_spec(spec)
spec.loader.exec_module(resources)


class ResourcesTests(unittest.TestCase):
    def test_descendants_keep_reparented_child_but_reject_reused_pid(self):
        table = {
            10: {"ppid": 1, "identity": "parent"},
            20: {"ppid": 10, "identity": "child"},
            30: {"ppid": 20, "identity": "grandchild"},
            40: {"ppid": 1, "identity": "new-process"},
        }
        self.assertEqual(resources.descendants(10, table, {40: "old-process"}), {10, 20, 30})
        del table[10]
        table[20]["ppid"] = 1
        self.assertEqual(resources.descendants(10, table, {20: "child", 40: "old-process"}), {20, 30})

    def test_cpu_time_and_empty_percentiles(self):
        self.assertEqual(resources.duration("01:02.50"), 62.5)
        self.assertEqual(resources.duration("1-02:03:04"), 93784)
        self.assertIsNone(resources.percentile([], .5))

    def test_screenshot_proofs_require_both_painted_windows_and_matching_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            events = []
            for filename in ["ready.png", "complete.png"]:
                file = pathlib.Path(directory) / filename
                file.write_bytes(b"recorded screenshot bytes: " + filename.encode())
                events.append({"event": "screenshot", "path": filename, "image_sha256": resources.sha256(file),
                               "bytes": file.stat().st_size, "content_pixels_verified": True,
                               "dark_content_pixels": 400, "luminance_levels": 32})
            self.assertTrue(resources.screenshot_proofs(events, directory)["verified"])
            self.assertFalse(resources.screenshot_proofs(events[:1], directory)["verified"])
            events[1]["dark_content_pixels"] = 10
            self.assertFalse(resources.screenshot_proofs(events, directory)["verified"])
            events[1]["dark_content_pixels"] = 400
            (pathlib.Path(directory) / "complete.png").write_bytes(b"modified screenshot")
            self.assertFalse(resources.screenshot_proofs(events, directory)["verified"])

    def test_old_screenshot_proof_without_image_hash_is_not_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            events = [{"event": "scenario-complete", "success": True}]
            self.assertFalse(resources.screenshot_proofs(events, directory)["verified"])
            file = pathlib.Path(directory) / "ready.png"
            file.write_bytes(b"screenshot")
            events.append({"event": "screenshot", "path": "ready.png", "sha256": resources.sha256(file),
                           "bytes": file.stat().st_size, "content_pixels_verified": True,
                           "dark_content_pixels": 400, "luminance_levels": 32})
            result = resources.screenshot_proofs(events, directory)
            self.assertFalse(result["verified"])
            self.assertIn("file hash", result["errors"][0])


if __name__ == "__main__":
    unittest.main()
