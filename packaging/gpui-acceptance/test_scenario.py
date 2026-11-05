import importlib.util
import json
import pathlib
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("scenario", pathlib.Path(__file__).with_name("scenario.py"))
scenario = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scenario)


class ScenarioTests(unittest.TestCase):
    def test_integrity_chain_rejects_modified_or_missing_input_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            file = pathlib.Path(directory) / "events.jsonl"
            journal = scenario.Journal(file)
            journal.write("launch", scenario="scroll")
            journal.write("action-observed", before={"pan_y": 0}, after={"pan_y": 200})
            journal.write("scenario-complete", scenario="scroll", success=True)
            events = [json.loads(line) for line in file.read_text().splitlines()]
            self.assertTrue(scenario.verify_journal(events))
            self.assertFalse(scenario.verify_journal(events[1:]))
            events[1]["after"]["pan_y"] = 0
            self.assertFalse(scenario.verify_journal(events))

    def test_a_completion_claim_without_a_hash_chain_is_not_evidence(self):
        self.assertFalse(scenario.verify_journal([]))
        self.assertFalse(scenario.verify_journal([{"event": "scenario-complete", "scenario": "scroll", "success": True}]))

    def test_screenshot_hash_remains_distinct_from_journal_digest(self):
        with tempfile.TemporaryDirectory() as directory:
            file = pathlib.Path(directory) / "events.jsonl"
            journal = scenario.Journal(file)
            journal.write("screenshot", path="ready.png", image_sha256="a" * 64, bytes=1200,
                          content_pixels_verified=True, dark_content_pixels=400, luminance_levels=25)
            events = [json.loads(line) for line in file.read_text().splitlines()]
            self.assertTrue(scenario.verify_journal(events))
            self.assertEqual(events[0]["image_sha256"], "a" * 64)
            self.assertNotEqual(events[0]["sha256"], events[0]["image_sha256"])
            with self.assertRaisesRegex(ValueError, "integrity or timing"):
                journal.write("screenshot", sha256="a" * 64)
            self.assertEqual(journal.sequence, 1)


if __name__ == "__main__":
    unittest.main()
