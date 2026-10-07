import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    "check_ci_needs", Path(__file__).with_name("check-ci-needs.py")
)
check_ci_needs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check_ci_needs)


class CheckNeedsTests(unittest.TestCase):
    def test_success(self):
        self.assertEqual(check_ci_needs.failures({"test": {"result": "success"}}, set()), [])

    def test_required_jobs_must_succeed(self):
        for result in ("failure", "cancelled", "skipped"):
            with self.subTest(result=result):
                self.assertEqual(
                    check_ci_needs.failures({"test": {"result": result}}, set()), ["test"]
                )

    def test_allow_skip_does_not_allow_failure_or_cancellation(self):
        for result in ("failure", "cancelled"):
            with self.subTest(result=result):
                self.assertEqual(
                    check_ci_needs.failures({"specs": {"result": result}}, {"specs"}),
                    ["specs"],
                )

    def test_only_explicitly_allowed_jobs_can_skip(self):
        self.assertEqual(
            check_ci_needs.failures(
                {"filter": {"result": "skipped"}, "specs": {"result": "skipped"}},
                {"specs"},
            ),
            ["filter"],
        )


if __name__ == "__main__":
    unittest.main()
