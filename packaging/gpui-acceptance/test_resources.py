import importlib.util
import pathlib
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


if __name__ == "__main__":
    unittest.main()
