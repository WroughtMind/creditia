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


if __name__ == "__main__":
    unittest.main()
