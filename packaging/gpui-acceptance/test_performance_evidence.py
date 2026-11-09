import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("performance_evidence", Path(__file__).with_name("performance-evidence.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ProcessReplay(unittest.TestCase):
    def test_exited_process_cpu_is_retained_and_reused_pid_is_distinct(self):
        rows = [
            {"processes": [{"pid": 1, "identity": "old", "cpu_seconds": .5, "rss_bytes": 10}],
             "rss_bytes": 10, "observed_cpu_seconds": .5},
            {"processes": [{"pid": 1, "identity": "new", "cpu_seconds": .25, "rss_bytes": 20}],
             "rss_bytes": 20, "observed_cpu_seconds": .75},
        ]
        summary = {"sample_count": 2, "observed_process_count": 2, "rss_peak_bytes": 20, "observed_cpu_seconds": .75}
        module.verify_samples(rows, summary)
        rows[1]["processes"][0]["identity"] = "old"
        with self.assertRaisesRegex(ValueError, "CPU total"):
            module.verify_samples(rows, summary)

    def test_raw_resource_values_must_reproduce_summary(self):
        rows = [{"processes": [{"pid": 5, "identity": "start", "cpu_seconds": 1, "rss_bytes": 30}],
                 "rss_bytes": 30, "observed_cpu_seconds": 1}]
        summary = {"sample_count": 1, "observed_process_count": 1, "rss_peak_bytes": 29, "observed_cpu_seconds": 1}
        with self.assertRaisesRegex(ValueError, "summary"):
            module.verify_samples(rows, summary)
        rows[0]["rss_bytes"] = 29
        with self.assertRaisesRegex(ValueError, "RSS sum"):
            module.verify_samples(rows, summary)


if __name__ == "__main__":
    unittest.main()
