import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    "check_advisory_features", Path(__file__).with_name("check-advisory-features.py")
)
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class AdvisoryFeatureTests(unittest.TestCase):
    def check_version(self, version, features):
        return check.vulnerable_formatters(
            {
                "packages": [{"id": "subscriber", "name": "tracing-subscriber", "version": version}],
                "resolve": {"nodes": [{"id": "subscriber", "features": features}]},
            }
        )

    def test_unaffected_arkworks_dependency(self):
        self.assertEqual(self.check_version("0.2.25", []), [])

    def test_enabling_old_formatter_fails(self):
        self.assertEqual(self.check_version("0.2.25", ["fmt"]), ["0.2.25"])

    def test_unpatched_application_formatter_fails(self):
        self.assertEqual(self.check_version("0.3.19", ["fmt"]), ["0.3.19"])

    def test_patched_formatter_is_allowed(self):
        self.assertEqual(self.check_version("0.3.20", ["fmt"]), [])
        self.assertEqual(self.check_version("0.3.23", ["fmt"]), [])


if __name__ == "__main__":
    unittest.main()
