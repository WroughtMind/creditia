import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("animation_smoke", Path(__file__).with_name("animation-smoke.py"))
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


class StreamingEvidenceTests(unittest.TestCase):
    def state(self):
        return {"display_scale": 2, "viewport_ready": True, "focused_rect": {"w": 1100},
                "animation": {"epoch": 4, "displayed_epoch": 4, "decode_ready": True,
                              "frame_pixels": 2200, "chunks": [{"width": 2200, "height": 900}],
                              "resident_bytes": 2200 * 900 * 4, "resident_frames": 1}}

    def test_current_physical_resolution_is_required(self):
        self.assertEqual(smoke.validate_frame(self.state())["required_pixels"], 2200)
        for change in ({"display_scale": 1}, {"viewport_ready": False}):
            state = self.state()
            state.update(change)
            with self.assertRaises(AssertionError):
                smoke.validate_frame(state)
        for change in ({"displayed_epoch": 3}, {"decode_ready": False}, {"frame_pixels": 2199}):
            state = self.state()
            state["animation"].update(change)
            with self.assertRaises(AssertionError):
                smoke.validate_frame(state)

    def test_chunk_and_total_residency_budgets_are_independent(self):
        for change in ({"chunks": [{"width": 2048, "height": 1025}]},
                       {"resident_bytes": 64 * smoke.MIB + 1}, {"resident_frames": 9},
                       {"resident_bytes": 1}):
            state = self.state()
            state["animation"].update(change)
            with self.assertRaises(AssertionError):
                smoke.validate_frame(state)

    def test_partial_json_lines_never_produce_fabricated_readiness(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / "events.jsonl"
            events = smoke.Events(file)
            self.assertEqual(events.read(), {})
            file.write_text('{"state":{"viewport_ready":false}}\n{"state":')
            self.assertEqual(events.read(), {"viewport_ready": False})
            self.assertEqual(events.sequence, 1)
            with file.open("a") as stream:
                stream.write('{"viewport_ready":true}}\n')
            self.assertEqual(events.read(), {"viewport_ready": True})
            self.assertEqual(events.sequence, 2)
            self.assertEqual(events.read(), {"viewport_ready": True})
            self.assertEqual(events.sequence, 2)

    def test_high_resolution_metadata_cannot_hide_a_missing_viewport_tile(self):
        state = self.state()
        state.update(art_width=2200, art_height=900, viewport={"width": 1360, "height": 820})
        state["focused_rect"].update(x=130, y=100)
        smoke.validate_frame(state)
        state["animation"]["chunks"] = [{"width": 2200, "height": 450,
                                           "region": {"x": 0, "y": 0, "w": 2200, "h": 450}}]
        with self.assertRaisesRegex(AssertionError, "visible gap"):
            smoke.validate_frame(state)

    def test_cadence_excludes_user_pauses_and_epoch_replacement_delays(self):
        def frame(at, epoch, index, playing=True):
            return {"time_ms": at, "animation": {"epoch": epoch, "displayed_epoch": epoch,
                    "index": index, "playing": playing, "decode_ready": True, "suspended": False}}
        rows = [frame(0, 1, 0), frame(16, 1, 1), frame(48, 1, 3), frame(49, 1, 3, False),
                frame(1000, 1, 3), frame(1016, 1, 4), frame(2000, 2, 0), frame(2016, 2, 1)]
        result = smoke.cadence(rows, 0, 3000)
        self.assertEqual(result["interval_count"], 4)
        self.assertEqual(result["interval_ms"], {"p50": 16, "p95": 32, "max": 32})
        self.assertEqual(result["skipped_sample_indices"], 1)
        self.assertEqual(result["gaps_over_250ms"], [])


if __name__ == "__main__":
    unittest.main()
